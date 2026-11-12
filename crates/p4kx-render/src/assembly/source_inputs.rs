// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{AssemblyScene, Item};

/// The actual Assembly piece from which a renderer part was built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AssemblyPieceSource {
    /// The ordinal in the Assembly's parts.
    Part(usize),
    /// The ordinal in the Assembly's bone attachment iterator.
    BoneAttachment(usize),
    /// A part of an interior container.
    Interior {
        /// The ordinal of the original container.
        container: usize,
        /// The ordinal within that container's parts.
        part: usize,
    },
}

/// A piece admitted for placement and used to compute the scene bounds.
#[derive(Debug)]
pub struct PlacedPieceInput<'scene> {
    ordinal: usize,
    source: AssemblyPieceSource,
    items: &'scene [Item],
}

impl PlacedPieceInput<'_> {
    /// Returns the accepted ordinal in the scene's parts.
    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// Returns the Assembly piece actually used by this scene.
    #[must_use]
    pub const fn source(&self) -> AssemblyPieceSource {
        self.source
    }

    /// Returns the actual library slots of the admitted draw ranges in order.
    pub fn material_slots(&self) -> impl Iterator<Item = usize> + '_ {
        self.items.iter().filter_map(|item| item.library_slot)
    }
}

impl AssemblyScene {
    /// Returns the actual pieces admitted by geometry, surface and placement checks.
    pub fn placed_inputs(&self) -> impl Iterator<Item = PlacedPieceInput<'_>> + '_ {
        self.draws.iter().filter_map(|draw| {
            let part = self.parts.get(draw.part)?;
            let items = self
                .items
                .get(draw.items.start as usize..draw.items.end as usize)?;
            Some(PlacedPieceInput {
                ordinal: draw.part,
                source: part.source,
                items,
            })
        })
    }
}
