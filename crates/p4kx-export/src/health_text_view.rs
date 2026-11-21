// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourceValueState};

/// A semantic text occurrence with its own origin, separate from shared storage origins.
#[derive(Debug, Clone, Copy)]
pub struct HealthTextView<'a> {
    value: Option<&'a str>,
    state: SourceValueState,
    origin: &'a FieldOrigin,
}

impl<'a> HealthTextView<'a> {
    pub(crate) const fn new(
        value: Option<&'a str>,
        state: SourceValueState,
        origin: &'a FieldOrigin,
    ) -> Self {
        Self {
            value,
            state,
            origin,
        }
    }

    /// Borrows the exact present semantic value.
    #[must_use]
    pub const fn value(&self) -> Option<&'a str> {
        self.value
    }

    /// Returns the semantic occurrence's original interpretation state.
    #[must_use]
    pub const fn state(&self) -> SourceValueState {
        self.state
    }

    /// Returns only this occurrence's contribution set and encryption marker.
    #[must_use]
    pub const fn origin(&self) -> &'a FieldOrigin {
        self.origin
    }
}
