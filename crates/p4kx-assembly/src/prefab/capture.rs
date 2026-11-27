// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;

use super::resolution_source::OwnedPrefabLocation;
use super::{PrefabFile, PrefabInstanceSource, PrefabResolutionSource, PrefabSelectionSource};

pub(super) type ObjectKey = (usize, usize, usize);

pub(super) struct Capture {
    pub(super) source: PrefabResolutionSource,
    pub(super) selections: HashMap<ObjectKey, Arc<PrefabSelectionSource>>,
}

impl Capture {
    pub(super) fn new(file: &Arc<PrefabFile>, prefab_index: usize) -> Self {
        Self {
            source: PrefabResolutionSource {
                root: OwnedPrefabLocation {
                    file: Arc::clone(file),
                    prefab_index,
                },
                entered: Vec::new(),
                issues: Vec::new(),
            },
            selections: HashMap::new(),
        }
    }

    pub(super) fn record_entered(&mut self, route: &PrefabInstanceSource) {
        let selection = &route.selection;
        let key = (
            selection.owner.file.slot(),
            selection.owner.prefab_index,
            selection.object_index,
        );
        if let Entry::Vacant(entry) = self.selections.entry(key) {
            entry.insert(Arc::clone(selection));
            self.source.entered.push(Arc::clone(selection));
        }
    }
}
