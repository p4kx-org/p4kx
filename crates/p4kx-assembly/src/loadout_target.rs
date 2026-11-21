// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::Guid;

use crate::{LoadoutError, LoadoutReferenceSelection};

/// An attachment that cannot be resolved: what was named and why it fails.
#[derive(Debug, Clone)]
pub struct Unresolved {
    pub(crate) target: Target,
    pub(crate) error: LoadoutError,
    pub(crate) reference: Option<LoadoutReferenceSelection>,
}

impl PartialEq for Unresolved {
    fn eq(&self, other: &Self) -> bool {
        self.target == other.target && self.error == other.error
    }
}

impl Eq for Unresolved {}

impl Unresolved {
    /// Borrows the actual original Reference selection, when this target was observed.
    #[must_use]
    pub const fn reference_selection(&self) -> Option<&LoadoutReferenceSelection> {
        self.reference.as_ref()
    }

    /// Returns what the Loadout names.
    #[must_use]
    pub const fn target(&self) -> &Target {
        &self.target
    }

    /// Returns why it cannot be resolved.
    #[must_use]
    pub const fn error(&self) -> &LoadoutError {
        &self.error
    }
}

/// What a Loadout names to attach to a port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A Record by Reference.
    Reference(Guid),
    /// An Entity by class name.
    ClassName(String),
    /// A Reference that cannot be read.
    Unreadable,
}
