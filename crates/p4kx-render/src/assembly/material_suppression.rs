// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{AssemblyPieceSource, AssemblyScene};

/// The actual material rule that suppressed a draw range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MaterialSuppressionReason {
    /// The material's hidden policy suppressed the range.
    Hidden,
    /// The renderer omitted a decal surface.
    Decal,
}

/// How the suppressed range affected the built scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MaterialSuppressionEffect {
    /// The part had no remaining range and did not enter the scene.
    WholePiece,
    /// The part entered the scene with this range omitted.
    DrawRange,
}

/// A selected library slot whose material suppressed a draw range.
#[derive(Debug, Clone, Copy)]
pub struct MaterialSuppression {
    pub(super) source: AssemblyPieceSource,
    pub(super) ordinal: usize,
    pub(super) slot: usize,
    pub(super) reason: MaterialSuppressionReason,
    pub(super) effect: MaterialSuppressionEffect,
}

impl MaterialSuppression {
    /// Returns the Assembly piece that supplied the selected material.
    #[must_use]
    pub const fn source(self) -> AssemblyPieceSource {
        self.source
    }

    /// Returns the part's ordinal in the scene.
    #[must_use]
    pub const fn ordinal(self) -> usize {
        self.ordinal
    }

    /// Returns the exact slot of the material library.
    #[must_use]
    pub const fn slot(self) -> usize {
        self.slot
    }

    /// Returns the applied material rule.
    #[must_use]
    pub const fn reason(self) -> MaterialSuppressionReason {
        self.reason
    }

    /// Returns the branch in which the range was omitted.
    #[must_use]
    pub const fn effect(self) -> MaterialSuppressionEffect {
        self.effect
    }
}

impl AssemblyScene {
    /// Returns the distinct suppressed library slots of each observed part.
    pub fn material_suppressions(&self) -> impl Iterator<Item = MaterialSuppression> + '_ {
        self.material_suppressions.iter().flatten().copied()
    }
}

#[cfg(test)]
#[path = "material_suppression_tests.rs"]
mod tests;
