// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The layer files the material libraries of an Interior could not resolve.

use p4kx_model::{MaterialStatus, UnresolvedLayers};

use super::Interior;

impl Interior {
    /// Returns the material libraries of the parts.
    pub(crate) fn libraries(&self) -> impl Iterator<Item = &MaterialStatus> {
        self.parts.iter().map(|part| &*part.materials)
    }

    /// Returns the layer files that the material libraries of the parts could not resolve; their
    /// materials keep the colours of their own file.
    #[must_use]
    pub fn unresolved_layers(&self) -> UnresolvedLayers {
        UnresolvedLayers::of(self.libraries())
    }
}
