// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;

use crate::loadout_file_cache::CachedDocument;
use crate::loadout_file_trace::FileTrace;
use crate::loadout_xml_use::XmlLoadoutAttributes;
use crate::resolver::{Entries, EntriesResult};
use crate::resolver_observation::Observation;
use crate::{
    LoadoutError, LoadoutFiles, LoadoutPath, LoadoutRequest, LoadoutResolver, LoadoutXmlClassUse,
    XmlPortReplacement, xml,
};

#[cfg(test)]
#[path = "resolver_xml_replacements_tests.rs"]
mod replacement_tests;

#[derive(Default)]
pub(crate) struct XmlCache {
    entries: HashMap<XmlCacheKey, CachedDocument<Result<ParsedEntries, LoadoutError>>>,
    include_read_budget: Option<usize>,
}

impl XmlCache {
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }
}

#[derive(Clone)]
struct ParsedEntries {
    entries: Entries,
    replacements: Option<Arc<[XmlPortReplacement]>>,
}

#[derive(PartialEq, Eq, Hash)]
struct XmlCacheKey {
    path: String,
    capture_paint: bool,
}

impl LoadoutResolver<'_> {
    /// Selects a maximum of new provider read attempts per XML Include resolution.
    ///
    /// The root and reused Includes cost no attempts. A failed provider read costs one attempt.
    /// `None` permits all Includes. An exhausted budget refuses the XML result.
    #[must_use]
    pub fn with_xml_include_read_budget(mut self, budget: Option<usize>) -> Self {
        self.xml.include_read_budget = budget;
        self.xml.clear();
        self
    }

    pub(crate) fn xml_entries(&mut self, path: &LoadoutPath) -> EntriesResult {
        let Some(files) = self.files else {
            return Err(LoadoutError::NoLoadoutFiles {
                path: path.to_string(),
            });
        };
        let caller = LoadoutRequest::Xml { path: path.clone() };
        let observation = self.observation.as_deref();
        let capture_paint = self.captures_vehicle_appearance();
        let key = XmlCacheKey {
            path: path.to_ascii_lowercase(),
            capture_paint,
        };
        let include_read_budget = self.xml.include_read_budget;
        let (parsed, original) = match self.xml.entries.entry(key) {
            Entry::Occupied(entry) => {
                let cached = entry.get();
                (
                    cached.reused(observation, &caller)?.clone(),
                    cached.xml_root().cloned(),
                )
            }
            Entry::Vacant(entry) => {
                let cached = entry.insert(CachedDocument::read(|trace| {
                    read_entries(
                        files,
                        path,
                        &caller,
                        observation,
                        capture_paint,
                        include_read_budget,
                        trace,
                    )
                }));
                (cached.value().clone(), cached.xml_root().cloned())
            }
        };
        let mut result = parsed.and_then(|parsed| {
            if let Some(replacements) = &parsed.replacements {
                let (Some(observer), Some(root)) = (observation, original.as_ref()) else {
                    if let Some(observer) = observation {
                        observer.stop();
                    }
                    return Err(LoadoutError::ObservationStopped);
                };
                observer.xml_port_replacements_used(root, path, replacements)?;
            }
            Ok(parsed.entries)
        });
        if let (Ok(entries), Some(original)) = (&mut result, original)
            && !entries.list.is_empty()
        {
            entries.xml_use = Some(entries.xml_use.as_ref().map_or_else(
                || LoadoutXmlClassUse::root(original, path, None),
                |usage| usage.with_caller(path),
            ));
        }
        attach_missing_file_caller(&mut result, caller);
        result
    }
}

fn attach_missing_file_caller(result: &mut EntriesResult, caller: LoadoutRequest) {
    match result {
        Ok(entries) if entries.issues.iter().any(LoadoutError::is_missing_file) => {
            let caller = Arc::new(caller);
            for issue in Arc::make_mut(&mut entries.issues) {
                issue.used_by(&caller);
            }
        }
        Err(error) if error.is_missing_file() => error.used_by(&Arc::new(caller)),
        _ => {}
    }
}

fn read_entries(
    files: &dyn LoadoutFiles,
    path: &LoadoutPath,
    caller: &LoadoutRequest,
    observation: Option<&Observation<'_>>,
    capture_paint: bool,
    include_read_budget: Option<usize>,
    trace: &mut FileTrace,
) -> Result<ParsedEntries, LoadoutError> {
    let attributes = capture_paint.then(|| Arc::new(XmlLoadoutAttributes::default()));
    let capture = attributes
        .as_ref()
        .map(|sites| Observation::new(sites.clone()));
    let loaded = xml::load_observed(
        files,
        path,
        caller,
        observation.or(capture.as_ref()),
        include_read_budget,
        trace,
    )?;
    let xml_use = attributes.and_then(|sites| {
        let original = trace.xml_root()?.clone();
        Some(LoadoutXmlClassUse::root(original, path, Some(sites)))
    });
    Ok(ParsedEntries {
        entries: Entries {
            list: loaded.entries,
            issues: Arc::from(loaded.issues),
            xml_use,
        },
        replacements: loaded.replacements,
    })
}
