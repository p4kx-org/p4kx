// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Assembly as the viewer draws it: every distinct model flattened once into shared buffers,
//! the placed parts as instances of those meshes, and the culling that picks the instances a
//! frame draws.

mod build;
mod build_place;
mod draw;
mod flatten;
mod frustum;
mod material_suppression;
mod mesh_node_input;
mod mips;
mod pieces;
mod source_inputs;
mod surface;
#[cfg(test)]
pub(crate) mod synthetic;
#[cfg(test)]
mod tests;
mod visibility;

use std::ops::Range;

use super::scene::{Bounds, Rows, Vertex};

pub use build::{BuildError, BuildOptions, Hooks, Progress};
pub use draw::{DrawList, GroupDraw, Instance, ObservedDrawList};
pub use material_suppression::{
    MaterialSuppression, MaterialSuppressionEffect, MaterialSuppressionReason,
};
pub use mesh_node_input::MeshNodeInput;
pub(crate) use mips::{Level, chain, level_count};
pub use source_inputs::{AssemblyPieceSource, PlacedPieceInput};
pub use visibility::hidden_mask;

/// The most vertices the shared meshes of a [`AssemblyScene`] hold; models that would pass it are
/// left out.
pub const MAX_VIEW_VERTICES: usize = 4_000_000;

/// The most triangles the shared meshes of a [`AssemblyScene`] hold.
pub const MAX_VIEW_TRIANGLES: usize = 8_000_000;

/// The most draw items, one per part and material, an [`AssemblyScene`] holds; further parts are
/// left out.
pub const MAX_VIEW_ITEMS: usize = 1 << 19;

/// The most bytes of texture pixels an [`AssemblyScene`] holds; materials beyond it keep their
/// plain colour.
pub const MAX_VIEW_TEXTURE_BYTES: usize = 256 * 1024 * 1024;

/// The mip level the viewer reads the textures of the materials at.
pub const VIEW_TEXTURE_MIP: u32 = 2;

/// The projected radius, in pixels, below which a part is not drawn.
pub const MIN_PART_PIXELS: f32 = 0.75;

/// Where a draw group's triangles are in the shared index buffer and how they are shaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Group {
    pub(crate) first_index: u32,
    pub(crate) index_count: u32,
    pub(crate) texture: Option<u32>,
    pub(crate) translucent: bool,
}

/// One material range of a part: the group that holds its triangles and the colour it is drawn in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Item {
    pub(crate) library_slot: Option<usize>,
    pub(crate) group: u32,
    pub(crate) colour: [f32; 4],
}

/// One part of the Assembly as the Loadout tree knows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartInfo {
    source: AssemblyPieceSource,
    name: String,
    path: String,
    parent: Option<usize>,
    triangles: usize,
    interior: bool,
}

impl PartInfo {
    /// Returns whether the part belongs to the Interior of the Entity rather than to its Loadout.
    #[must_use]
    pub const fn is_interior(&self) -> bool {
        self.interior
    }

    /// Returns the part's name, `<port>/<entity>`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the path of the part's port from the root, port names joined by `/`; empty for
    /// the root.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the index of the parent part, or `None` for the root.
    #[must_use]
    pub const fn parent(&self) -> Option<usize> {
        self.parent
    }

    /// Returns the number of triangles of the part's model, 0 for a part without one.
    #[must_use]
    pub const fn triangles(&self) -> usize {
        self.triangles
    }
}

/// A part that is drawn: where it is, how far its box reaches and which items it has.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PartDraw {
    pub(crate) part: usize,
    pub(crate) mesh: usize,
    pub(crate) bounds: Bounds,
    pub(crate) world: Rows,
    pub(crate) centre: [f32; 3],
    pub(crate) radius: f32,
    pub(crate) items: Range<u32>,
}

/// An image the materials of the Assembly are drawn with.
#[derive(Debug)]
pub(crate) struct ViewTexture {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pixels: std::sync::Mutex<(Vec<u8>, Vec<Level>)>,
}

impl ViewTexture {
    /// Keeps `rgba` and the levels `mips` of its chain, built beforehand.
    pub(crate) const fn new(width: u32, height: u32, rgba: Vec<u8>, mips: Vec<Level>) -> Self {
        Self {
            width,
            height,
            pixels: std::sync::Mutex::new((rgba, mips)),
        }
    }

    /// Hands the pixels and the mip levels over for the upload, leaving none on the CPU; later
    /// calls get none.
    pub(crate) fn take_pixels(&self) -> (Vec<u8>, Vec<Level>) {
        self.pixels
            .lock()
            .map(|mut pixels| std::mem::take(&mut *pixels))
            .unwrap_or_default()
    }
}

/// What building the scene left out or could not do.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Left {
    /// Parts whose model is above the limits of the scene or has no finite coordinates.
    pub parts: usize,
    /// Textures that could not be read or did not fit the budget; their materials keep the
    /// plain colour.
    pub textures: usize,
}

/// The Assembly flattened for drawing: shared meshes and the parts placed as instances.
///
/// What is drawn is the [`p4kx_assembly::Assembly`] the export writes, at the same level of
/// detail, with the same placement, materials and palettes.
#[derive(Debug)]
pub struct AssemblyScene {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    groups: Vec<Group>,
    items: Vec<Item>,
    draws: Vec<PartDraw>,
    parts: Vec<PartInfo>,
    material_suppressions: Option<Vec<MaterialSuppression>>,
    mesh_node_inputs: Option<mesh_node_input::MeshNodeInputs>,
    textures: Vec<ViewTexture>,
    meshes: usize,
    bounds: Bounds,
    left: Left,
    serial: u64,
}

impl AssemblyScene {
    /// Returns the vertices of the shared meshes.
    #[must_use]
    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    /// Returns the triangle list of the shared meshes: three vertex indices per triangle.
    #[must_use]
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    pub(crate) fn groups(&self) -> &[Group] {
        &self.groups
    }

    pub(crate) fn is_translucent(&self, group: u32) -> bool {
        self.groups
            .get(group as usize)
            .is_some_and(|group| group.translucent)
    }

    pub(crate) fn items(&self) -> &[Item] {
        &self.items
    }

    pub(crate) fn draws(&self) -> &[PartDraw] {
        &self.draws
    }

    pub(crate) fn textures(&self) -> &[ViewTexture] {
        &self.textures
    }

    /// Returns every part of the Assembly, the root first, every part after its parent.
    #[must_use]
    pub fn parts(&self) -> &[PartInfo] {
        &self.parts
    }

    /// Returns the number of parts that are drawn when none is hidden.
    #[must_use]
    pub const fn drawn_parts(&self) -> usize {
        self.draws.len()
    }

    /// Returns how many parts are drawn and how many triangles they draw together when none is
    /// hidden or culled, counting the parts of the Interior only when `interior` is set.
    #[must_use]
    pub fn drawn(&self, interior: bool) -> (usize, usize) {
        self.draws
            .iter()
            .filter_map(|draw| self.parts.get(draw.part))
            .filter(|part| interior || !part.is_interior())
            .fold((0, 0), |(parts, triangles), part| {
                (parts + 1, triangles + part.triangles())
            })
    }

    /// Returns the number of parts of the Interior that are drawn when none is hidden.
    #[must_use]
    pub fn interior_parts(&self) -> usize {
        self.drawn(true).0 - self.drawn(false).0
    }

    /// Returns the number of distinct meshes, which the parts share.
    #[must_use]
    pub const fn meshes(&self) -> usize {
        self.meshes
    }

    /// Returns the number of distinct meshes the drawn parts use, counting the parts of the
    /// Interior only when `interior` is set.
    #[must_use]
    pub fn meshes_of(&self, interior: bool) -> usize {
        let mut used: Vec<usize> = self
            .draws
            .iter()
            .filter(|draw| self.is_counted(draw, interior))
            .map(|draw| draw.mesh)
            .collect();
        used.sort_unstable();
        used.dedup();
        used.len()
    }

    /// Returns the box around the drawn parts, counting the parts of the Interior only when
    /// `interior` is set.
    #[must_use]
    pub fn bounds_of(&self, interior: bool) -> Bounds {
        if interior {
            return self.bounds;
        }
        let mut bounds = Bounds::EMPTY;
        for draw in self
            .draws
            .iter()
            .filter(|draw| self.is_counted(draw, false))
        {
            bounds.union(&draw.bounds);
        }
        if bounds.is_valid() {
            bounds
        } else {
            self.bounds
        }
    }

    fn is_counted(&self, draw: &PartDraw, interior: bool) -> bool {
        interior
            || self
                .parts
                .get(draw.part)
                .is_some_and(|part| !part.is_interior())
    }

    /// Returns the number of draw items, one per part and material.
    #[must_use]
    pub const fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns the number of distinct draw groups: a mesh range with its texture.
    #[must_use]
    pub const fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Returns the number of distinct textures.
    #[must_use]
    pub const fn texture_count(&self) -> usize {
        self.textures.len()
    }

    /// Returns the number of triangles stored, each shared mesh once.
    #[must_use]
    pub const fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Returns the number of triangles the parts draw together when none is hidden or culled.
    #[must_use]
    pub fn placed_triangles(&self) -> usize {
        self.draws
            .iter()
            .filter_map(|draw| self.parts.get(draw.part))
            .map(PartInfo::triangles)
            .sum()
    }

    /// Returns the box around every placed part.
    #[must_use]
    pub const fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    /// Returns what the scene leaves out.
    #[must_use]
    pub const fn left(&self) -> Left {
        self.left
    }

    /// Returns a number no other scene of this process has; the GPU caches are keyed by it.
    #[must_use]
    pub const fn serial(&self) -> u64 {
        self.serial
    }
}
