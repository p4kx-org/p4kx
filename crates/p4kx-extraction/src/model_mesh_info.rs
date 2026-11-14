// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{Mesh, Submesh};
use serde::Serialize;

/// One mesh of a level of detail.
#[derive(Debug, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the JSON has one flag per optional vertex stream of a mesh"
)]
pub struct MeshInfo {
    node: usize,
    vertices: usize,
    triangles: usize,
    normals: bool,
    normals_computed: bool,
    tangents: bool,
    colours: bool,
    second_uvs: bool,
    non_finite_uvs: usize,
    skinned: bool,
    submeshes: Vec<SubmeshInfo>,
}

impl MeshInfo {
    pub(super) fn of(mesh: &Mesh) -> Self {
        Self {
            node: mesh.node,
            vertices: mesh.positions.len(),
            triangles: mesh.indices.len() / 3,
            normals: mesh.normals.is_some(),
            normals_computed: mesh.normals_computed,
            tangents: mesh.tangents.is_some(),
            colours: mesh.colours.is_some(),
            second_uvs: mesh.second_uvs.is_some(),
            non_finite_uvs: mesh.non_finite_uvs,
            skinned: mesh.influences.is_some(),
            submeshes: mesh.submeshes.iter().map(SubmeshInfo::of).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct SubmeshInfo {
    material_id: u16,
    first_index: usize,
    index_count: usize,
}

impl SubmeshInfo {
    const fn of(submesh: &Submesh) -> Self {
        Self {
            material_id: submesh.material_id,
            first_index: submesh.first_index,
            index_count: submesh.index_count,
        }
    }
}
