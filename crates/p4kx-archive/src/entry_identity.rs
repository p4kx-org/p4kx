// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

/// Admitted container Entry indices relative to one shared root Archive demand.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub(crate) struct ContainerChain {
    entries: Arc<[u32]>,
}

impl ContainerChain {
    pub(crate) fn child(&self, entry: u32) -> Self {
        let mut entries = Vec::with_capacity(self.entries.len() + 1);
        entries.extend_from_slice(&self.entries);
        entries.push(entry);
        Self {
            entries: entries.into(),
        }
    }

    pub(crate) fn entry(&self, index: u32) -> PhysicalEntryId {
        PhysicalEntryId {
            containers: self.clone(),
            index,
        }
    }
}

/// One physical Entry position, including every containing Entry position.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PhysicalEntryId {
    containers: ContainerChain,
    index: u32,
}
