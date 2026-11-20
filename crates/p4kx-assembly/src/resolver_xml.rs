// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::hash_map::Entry;
use std::sync::Arc;

use crate::loadout_file_cache::CachedDocument;
use crate::loadout_file_trace::FileTrace;
use crate::loadout_xml_use::XmlLoadoutAttributes;
use crate::resolver::{Entries, EntriesResult};
use crate::resolver_observation::Observation;
use crate::{
    LoadoutError, LoadoutFiles, LoadoutPath, LoadoutRequest, LoadoutResolver, LoadoutXmlClassUse,
    xml,
};

#[derive(PartialEq, Eq, Hash)]
pub(crate) struct XmlCacheKey {
    path: String,
    capture_paint: bool,
}

impl LoadoutResolver<'_> {
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
        let (mut result, original) = match self.xml.entry(key) {
            Entry::Occupied(entry) => {
                let cached = entry.get();
                (
                    cached.reused(observation, &caller)?.clone(),
                    cached.xml_root().cloned(),
                )
            }
            Entry::Vacant(entry) => {
                let cached = entry.insert(CachedDocument::read(|trace| {
                    read_entries(files, path, &caller, observation, capture_paint, trace)
                }));
                (cached.value().clone(), cached.xml_root().cloned())
            }
        };
        if let (Ok(entries), Some(original)) = (&mut result, original)
            && !entries.list.is_empty()
        {
            entries.xml_use = Some(entries.xml_use.as_ref().map_or_else(
                || LoadoutXmlClassUse::root(original, path, None),
                |usage| usage.with_caller(path),
            ));
        }
        match &mut result {
            Ok(entries) if entries.issues.iter().any(LoadoutError::is_missing_file) => {
                let caller = Arc::new(caller);
                for issue in Arc::make_mut(&mut entries.issues) {
                    issue.used_by(&caller);
                }
            }
            Err(error) if error.is_missing_file() => error.used_by(&Arc::new(caller)),
            _ => {}
        }
        result
    }
}

fn read_entries(
    files: &dyn LoadoutFiles,
    path: &LoadoutPath,
    caller: &LoadoutRequest,
    observation: Option<&Observation<'_>>,
    capture_paint: bool,
    trace: &mut FileTrace,
) -> EntriesResult {
    let attributes = capture_paint.then(|| Arc::new(XmlLoadoutAttributes::default()));
    let capture = attributes
        .as_ref()
        .map(|sites| Observation::new(sites.clone()));
    let loaded = xml::load_observed(files, path, caller, observation.or(capture.as_ref()), trace)?;
    let xml_use = attributes.and_then(|sites| {
        let original = trace.xml_root()?.clone();
        Some(LoadoutXmlClassUse::root(original, path, Some(sites)))
    });
    Ok(Entries {
        list: Arc::from(loaded.entries),
        issues: Arc::from(loaded.issues),
        xml_use,
    })
}
