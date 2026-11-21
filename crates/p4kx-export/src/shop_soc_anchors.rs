// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use crate::catalog_read::original::AuthoredEntryPath;
use crate::shop_registry::is_guid;

/// The identifiers a shop component can be tied to inside one container: the `CryGUID`s of its
/// `entitycomponentshop.xml` and the names of its `SCShop` objects, both of which the container
/// states for the same Entities.
#[derive(Debug, Default)]
pub(crate) struct Anchors {
    guids: HashSet<String>,
    names: HashMap<String, String>,
    guid_selectors: HashMap<String, Vec<AuthoredEntryPath>>,
    name_selectors: HashMap<String, Vec<AuthoredEntryPath>>,
}

impl Anchors {
    /// Anchors for the lower-case `guids`, with the `names` of the objects that are unique in the
    /// container, each mapped to its lower-case `CryGUID`.
    #[cfg(test)]
    pub(crate) fn new(guids: &[String], names: HashMap<String, String>) -> Self {
        let guids: HashSet<String> = guids.iter().cloned().collect();
        let names = names
            .into_iter()
            .filter(|(_, guid)| guids.contains(guid))
            .collect();
        Self {
            guids,
            names,
            ..Self::default()
        }
    }

    pub(crate) fn captured(
        guids: Vec<(String, AuthoredEntryPath)>,
        names: HashMap<String, (String, Vec<AuthoredEntryPath>)>,
    ) -> Self {
        let mut captured: HashMap<String, Vec<AuthoredEntryPath>> = HashMap::new();
        for (guid, path) in guids {
            captured.entry(guid).or_default().push(path);
        }
        let guids = captured.keys().cloned().collect();
        let mut selectors = HashMap::new();
        let names = names
            .into_iter()
            .filter_map(|(name, (guid, mut inputs))| {
                inputs.extend(captured.get(&guid)?.iter().cloned());
                selectors.insert(name.clone(), inputs);
                Some((name, guid))
            })
            .collect();
        Self {
            guids,
            names,
            guid_selectors: captured,
            name_selectors: selectors,
        }
    }

    pub(crate) fn guid_receipts(&self, guid: &str) -> &[AuthoredEntryPath] {
        self.guid_selectors.get(guid).map_or(&[], Vec::as_slice)
    }

    pub(super) fn entity(&self, text: &str) -> Option<Anchor<'_>> {
        if let Some(guid) = self.names.get(text) {
            return Some(Anchor {
                entity: guid,
                selectors: self.name_selectors.get(text).map_or(&[], Vec::as_slice),
            });
        }
        if is_guid(text) {
            let lower = text.to_ascii_lowercase();
            let guid = self.guids.get(&lower)?;
            return Some(Anchor {
                entity: guid,
                selectors: self.guid_selectors.get(guid).map_or(&[], Vec::as_slice),
            });
        }
        None
    }
}

pub(super) struct Anchor<'anchors> {
    pub(super) entity: &'anchors str,
    pub(super) selectors: &'anchors [AuthoredEntryPath],
}
