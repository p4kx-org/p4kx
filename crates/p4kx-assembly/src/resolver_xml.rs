// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::hash_map::Entry;
use std::sync::Arc;

use crate::loadout_file_cache::CachedDocument;
use crate::resolver::{Entries, EntriesResult};
use crate::{LoadoutError, LoadoutPath, LoadoutRequest, LoadoutResolver, LoadoutXmlClassUse, xml};

impl LoadoutResolver<'_> {
    pub(crate) fn xml_entries(&mut self, path: &LoadoutPath) -> EntriesResult {
        let Some(files) = self.files else {
            return Err(LoadoutError::NoLoadoutFiles {
                path: path.to_string(),
            });
        };
        let caller = LoadoutRequest::Xml { path: path.clone() };
        let observation = self.observation.as_deref();
        let (mut result, original) = match self.xml.entry(path.to_ascii_lowercase()) {
            Entry::Occupied(entry) => {
                let cached = entry.get();
                (
                    cached.reused(observation, &caller)?.clone(),
                    cached.xml_root().cloned(),
                )
            }
            Entry::Vacant(entry) => {
                let cached = entry.insert(CachedDocument::read(|trace| {
                    xml::load_observed(files, path, &caller, observation, trace).map(|loaded| {
                        Entries {
                            list: Arc::from(loaded.entries),
                            issues: Arc::from(loaded.issues),
                            xml_use: None,
                        }
                    })
                }));
                (cached.value().clone(), cached.xml_root().cloned())
            }
        };
        if let (Ok(entries), Some(original)) = (&mut result, original)
            && !entries.list.is_empty()
        {
            entries.xml_use = Some(LoadoutXmlClassUse::root(original, path));
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
