// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::Model;

use super::super::scene::{Bounds, IDENTITY, Part, Vertex, world_transforms};
use super::super::skeleton::Skeleton;
use super::mesh_node_input::{MeshNodeCapture, MeshNodeInput, MeshNodeTrace};

/// A range of a flattened model drawn with one material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Range {
    pub(super) first_index: u32,
    pub(super) index_count: u32,
    pub(super) material: u16,
}

/// A model at one level of detail as one triangle list in model space, node transforms applied.
#[derive(Debug)]
pub(super) struct Flat {
    pub(super) vertices: Vec<Vertex>,
    pub(super) indices: Vec<u32>,
    pub(super) ranges: Vec<Range>,
    pub(super) bounds: Bounds,
    pub(super) mesh_node_trace: Option<MeshNodeTrace>,
}

/// Flattens the level of `model` that `wanted` selects, or returns `None` for a model without
/// triangles or with more than `u32` indices.
pub(super) fn flatten(model: &Model, wanted: u32, capture: MeshNodeCapture) -> Option<Flat> {
    let lod = model.nearest_lod(wanted)?;
    let world = world_transforms(&model.nodes);
    let skeleton = Skeleton::of(model.bones_of(lod));
    let bones = skeleton.as_ref().map_or(0, Skeleton::bone_count);
    let mut flat = Flat {
        vertices: Vec::with_capacity(lod.vertex_count()),
        indices: Vec::with_capacity(lod.triangle_count().saturating_mul(3)),
        ranges: Vec::new(),
        bounds: Bounds::EMPTY,
        mesh_node_trace: matches!(capture, MeshNodeCapture::Enabled).then(MeshNodeTrace::default),
    };
    for (ordinal, mesh) in lod.meshes.iter().enumerate() {
        let matrix = world.get(mesh.node);
        let part = Part::of(mesh, matrix.unwrap_or(&IDENTITY), bones);
        let node = (flat.mesh_node_trace.is_some()
            && matrix.is_some()
            && part.skin.is_empty()
            && !part.vertices.is_empty())
        .then_some(MeshNodeInput {
            level: lod.level,
            mesh: ordinal,
            node: mesh.node,
        });
        let base = u32::try_from(flat.vertices.len()).ok()?;
        let first = u32::try_from(flat.indices.len()).ok()?;
        let count = u32::try_from(part.indices.len()).ok()?;
        flat.include_bounds(&part.bounds, node);
        flat.indices
            .extend(part.indices.iter().map(|index| index.saturating_add(base)));
        flat.vertices.extend(part.vertices);
        if mesh.submeshes.is_empty() {
            flat.range(
                Range {
                    first_index: first,
                    index_count: count - count % 3,
                    material: 0,
                },
                node,
            );
        }
        for submesh in &mesh.submeshes {
            let start = u32::try_from(submesh.first_index).ok()?;
            let length = u32::try_from(submesh.index_count).ok()?;
            let end = start.saturating_add(length).min(count);
            let length = end.saturating_sub(start);
            if length >= 3 {
                flat.range(
                    Range {
                        first_index: first.checked_add(start)?,
                        index_count: length - length % 3,
                        material: submesh.material_id,
                    },
                    node,
                );
            }
        }
    }
    (!flat.indices.is_empty() && flat.bounds.is_valid()).then_some(flat)
}

impl Flat {
    fn include_bounds(&mut self, bounds: &Bounds, node: Option<MeshNodeInput>) {
        let before = self.mesh_node_trace.as_ref().map(|_| self.bounds);
        self.bounds.union(bounds);
        if let Some((trace, before)) = self.mesh_node_trace.as_mut().zip(before) {
            trace.update_bounds(before, self.bounds, node);
        }
    }

    fn range(&mut self, range: Range, node: Option<MeshNodeInput>) {
        if let Some(trace) = self.mesh_node_trace.as_mut() {
            trace.ranges.push(node.filter(|_| range.index_count > 0));
        }
        self.ranges.push(range);
    }
}

#[cfg(test)]
#[path = "flatten_tests.rs"]
mod tests;
