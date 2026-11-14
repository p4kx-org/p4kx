// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::{Display, Formatter, Result as FmtResult};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use p4kx_dataforge::Guid;

/// An opaque handle minted by one actual original Reference getter.
#[derive(Debug, Clone)]
pub struct LoadoutReferenceSelection {
    identity: Arc<()>,
}

impl LoadoutReferenceSelection {
    pub(crate) fn new() -> Self {
        Self {
            identity: Arc::new(()),
        }
    }
}

impl PartialEq for LoadoutReferenceSelection {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }
}

impl Eq for LoadoutReferenceSelection {}

impl Hash for LoadoutReferenceSelection {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.identity).hash(state);
    }
}

/// A copied GUID paired with the selection made while its original value was borrowed.
#[derive(Debug, Clone)]
pub struct LoadoutReference {
    guid: Guid,
    selection: Option<LoadoutReferenceSelection>,
}

impl LoadoutReference {
    /// Creates a GUID without an original source-authorship claim.
    #[must_use]
    pub const fn unbound(guid: Guid) -> Self {
        Self {
            guid,
            selection: None,
        }
    }

    /// Returns the GUID named by the getter or configuration.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }

    /// Borrows the actual original Reference selection, when observed.
    #[must_use]
    pub const fn selection(&self) -> Option<&LoadoutReferenceSelection> {
        self.selection.as_ref()
    }

    pub(crate) fn with_selection(mut self, selection: Option<LoadoutReferenceSelection>) -> Self {
        self.selection = selection;
        self
    }
}

impl PartialEq for LoadoutReference {
    fn eq(&self, other: &Self) -> bool {
        self.guid == other.guid
    }
}

impl Eq for LoadoutReference {}

impl Display for LoadoutReference {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(&self.guid, formatter)
    }
}

#[cfg(test)]
#[path = "loadout_reference_tests.rs"]
mod tests;
