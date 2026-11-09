// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::hash_map::Entry;
use std::sync::Arc;

use crate::loadout_file_cache::CachedDocument;
use crate::resolver::{Entries, EntriesResult};
use crate::{LoadoutError, LoadoutResolver, xml};

impl LoadoutResolver<'_> {
    pub(crate) fn xml_entries(&mut self, path: &str) -> EntriesResult {
        let Some(files) = self.files else {
            return Err(LoadoutError::NoLoadoutFiles {
                path: path.to_owned(),
            });
        };
        let observation = self.observation.as_deref();
        match self.xml.entry(path.to_ascii_lowercase()) {
            Entry::Occupied(entry) => entry.get().reused(observation)?.clone(),
            Entry::Vacant(entry) => entry
                .insert(CachedDocument::read(|trace| {
                    xml::load_observed(files, path, observation, trace).map(|loaded| Entries {
                        list: Arc::from(loaded.entries),
                        issues: Arc::from(loaded.issues),
                    })
                }))
                .value()
                .clone(),
        }
    }
}
