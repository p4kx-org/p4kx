// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::{Display, Formatter, Result as FmtResult};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// An opaque handle minted by one actual original class-name getter.
#[derive(Debug, Clone)]
pub struct LoadoutClassSelection {
    identity: Arc<()>,
}

impl LoadoutClassSelection {
    pub(crate) fn new() -> Self {
        Self {
            identity: Arc::new(()),
        }
    }
}

impl PartialEq for LoadoutClassSelection {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }
}
impl Eq for LoadoutClassSelection {}
impl Hash for LoadoutClassSelection {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.identity).hash(state);
    }
}

/// An authored class name paired with its actual source getter when observed.
#[derive(Debug, Clone)]
pub struct LoadoutClassName {
    value: String,
    selection: Option<LoadoutClassSelection>,
}

impl LoadoutClassName {
    /// Creates configured text without an original source-authorship claim.
    #[must_use]
    pub fn unbound(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            selection: None,
        }
    }

    /// Borrows the exact class text accepted by the getter.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Borrows this original class-name selection, when observed.
    #[must_use]
    pub const fn selection(&self) -> Option<&LoadoutClassSelection> {
        self.selection.as_ref()
    }

    pub(crate) fn captured(value: &str, selection: Option<LoadoutClassSelection>) -> Self {
        Self {
            value: value.to_owned(),
            selection,
        }
    }
}

impl PartialEq for LoadoutClassName {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for LoadoutClassName {}
impl Display for LoadoutClassName {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(self.value())
    }
}

#[cfg(test)]
#[path = "loadout_class_test_source.rs"]
mod test_source;

#[cfg(test)]
#[path = "loadout_class_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "loadout_class_xml_tests.rs"]
mod xml_tests;
