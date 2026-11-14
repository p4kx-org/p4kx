// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;

use p4kx_assembly::GearAnimationKey;

use super::{AssemblyScene, Item, MeshNodeInput};

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
    bound_nodes: &'scene [MeshNodeInput],
    group_nodes: &'scene [Option<MeshNodeInput>],
    bound_gear_keys: &'scene [GearAnimationKey],
    group_gear_keys: Option<&'scene HashMap<u32, Arc<[GearAnimationKey]>>>,
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

    /// Returns the mesh node transforms that set this piece's local bounds.
    pub fn bound_mesh_nodes(&self) -> impl Iterator<Item = MeshNodeInput> + '_ {
        self.bound_nodes.iter().copied()
    }

    /// Returns the mesh node transforms of the accepted draw ranges.
    pub fn drawn_mesh_nodes(&self) -> impl Iterator<Item = MeshNodeInput> + '_ {
        self.items
            .iter()
            .filter_map(|item| self.group_nodes.get(item.group as usize).copied().flatten())
    }

    /// Returns the gear animation keys used by the vertices setting local bounds.
    pub fn bound_gear_keys(&self) -> impl Iterator<Item = GearAnimationKey> + '_ {
        self.bound_gear_keys.iter().copied()
    }

    /// Returns the gear animation keys used by the admitted triangle ranges.
    pub fn drawn_gear_keys(&self) -> impl Iterator<Item = GearAnimationKey> + '_ {
        self.items
            .iter()
            .filter_map(|item| self.group_gear_keys?.get(&item.group))
            .flat_map(|keys| keys.iter().copied())
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
            let observed = self.mesh_node_inputs.as_ref();
            Some(PlacedPieceInput {
                ordinal: draw.part,
                source: part.source,
                items,
                bound_nodes: observed
                    .and_then(|inputs| inputs.bounds.get(draw.mesh))
                    .map_or(&[], Vec::as_slice),
                group_nodes: observed.map_or(&[], |inputs| inputs.groups.as_slice()),
                bound_gear_keys: observed
                    .and_then(|inputs| inputs.gear_bounds.get(&draw.mesh))
                    .map_or(&[], Vec::as_slice),
                group_gear_keys: observed.map(|inputs| &inputs.gear_groups),
            })
        })
    }
}
