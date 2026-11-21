// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::GearPoseInput;
use p4kx_extraction::model::{Mesh, Model};

use super::super::scene::{Bounds, IDENTITY, Part, Vertex, world_transforms};
use super::super::skeleton::Skeleton;
use super::gear_key_trace::{GearKeyTrace, GearMeshInput};
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
    pub(super) gear_key_trace: Option<GearKeyTrace>,
}

/// Flattens the level of `model` that `wanted` selects, or returns `None` for a model without
/// triangles or with more than `u32` indices.
pub(super) fn flatten(
    model: &Model,
    wanted: u32,
    capture: MeshNodeCapture,
    gear: Option<GearPoseInput<'_>>,
) -> Option<Flat> {
    let lod = model.nearest_lod(wanted)?;
    let gear = gear.and_then(|pose| {
        let level = model
            .lods
            .iter()
            .position(|candidate| std::ptr::eq(candidate, lod))?;
        Some((pose, level))
    });
    let world = world_transforms(&model.nodes);
    let skeleton = Skeleton::of(model.bones_of(lod));
    let bones = skeleton.as_ref().map_or(0, Skeleton::bone_count);
    let mut flat = Flat {
        vertices: Vec::with_capacity(lod.vertex_count()),
        indices: Vec::with_capacity(lod.triangle_count().saturating_mul(3)),
        ranges: Vec::new(),
        bounds: Bounds::EMPTY,
        mesh_node_trace: matches!(capture, MeshNodeCapture::Enabled).then(MeshNodeTrace::default),
        gear_key_trace: gear.map(|_| GearKeyTrace::default()),
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
        let pose = gear.map(|(pose, lod_ordinal)| GearMeshInput {
            pose,
            lod_ordinal,
            mesh: ordinal,
        });
        flat.append_mesh(mesh, part, node, pose)?;
    }
    (!flat.indices.is_empty() && flat.bounds.is_valid()).then_some(flat)
}

impl Flat {
    fn append_mesh(
        &mut self,
        mesh: &Mesh,
        part: Part,
        node: Option<MeshNodeInput>,
        pose: Option<GearMeshInput<'_>>,
    ) -> Option<()> {
        let base = u32::try_from(self.vertices.len()).ok()?;
        let first = u32::try_from(self.indices.len()).ok()?;
        let count = u32::try_from(part.indices.len()).ok()?;
        let before = self.bounds;
        self.include_bounds(&part.bounds, node);
        if let Some((trace, pose)) = self.gear_key_trace.as_mut().zip(pose) {
            trace.update_bounds(pose, &part.vertices, before, self.bounds);
        }
        self.indices
            .extend(part.indices.iter().map(|index| index.saturating_add(base)));
        self.vertices.extend(part.vertices);
        if mesh.submeshes.is_empty() {
            self.range(
                Range {
                    first_index: first,
                    index_count: count - count % 3,
                    material: 0,
                },
                node,
                pose,
                (first, &part.indices),
            );
        }
        for submesh in &mesh.submeshes {
            let start = u32::try_from(submesh.first_index).ok()?;
            let length = u32::try_from(submesh.index_count).ok()?;
            let end = start.saturating_add(length).min(count);
            let length = end.saturating_sub(start);
            if length >= 3 {
                self.range(
                    Range {
                        first_index: first.checked_add(start)?,
                        index_count: length - length % 3,
                        material: submesh.material_id,
                    },
                    node,
                    pose,
                    (first, &part.indices),
                );
            }
        }
        Some(())
    }

    fn include_bounds(&mut self, bounds: &Bounds, node: Option<MeshNodeInput>) {
        let before = self.mesh_node_trace.as_ref().map(|_| self.bounds);
        self.bounds.union(bounds);
        if let Some((trace, before)) = self.mesh_node_trace.as_mut().zip(before) {
            trace.update_bounds(before, self.bounds, node);
        }
    }

    fn range(
        &mut self,
        range: Range,
        node: Option<MeshNodeInput>,
        pose: Option<GearMeshInput<'_>>,
        (first, indices): (u32, &[u32]),
    ) {
        if let Some(trace) = self.mesh_node_trace.as_mut() {
            trace.ranges.push(node.filter(|_| range.index_count > 0));
        }
        if let Some((trace, pose)) = self.gear_key_trace.as_mut().zip(pose) {
            let start = (range.first_index - first) as usize;
            let count = range.index_count as usize;
            trace.range(pose, indices.get(start..start + count).unwrap_or_default());
        }
        self.ranges.push(range);
    }
}

#[cfg(test)]
#[path = "flatten_tests.rs"]
mod tests;
