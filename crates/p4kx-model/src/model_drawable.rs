// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{Lod, Mesh, Submesh};
use crate::{MaterialStatus, MeshOrigin};

impl Lod {
    /// Returns the material IDs of geometry-bearing submeshes in mesh order, with repeats.
    pub fn drawable_material_ids(&self) -> impl Iterator<Item = u16> + '_ {
        self.meshes.iter().flat_map(|mesh| {
            mesh.submeshes
                .iter()
                .filter_map(|submesh| mesh.submesh_indices(submesh).map(|_| submesh.material_id))
        })
    }

    /// Returns whether one of this level's drawable material IDs requires a fallback.
    #[must_use]
    pub fn requires_material_fallback(&self, materials: &MaterialStatus) -> bool {
        let library = materials.library();
        self.drawable_material_ids().any(|id| {
            library
                .is_none_or(|library| library.material(id).is_none() || library.is_empty_slot(id))
        })
    }
}

impl Mesh {
    /// Returns the physical IVO source that supplied the mesh's stream vertices.
    #[must_use]
    pub const fn origin(&self) -> Option<MeshOrigin> {
        self.origin
    }

    /// Returns the nonempty index range of this submesh when the mesh carries geometry.
    #[must_use]
    pub fn submesh_indices(&self, submesh: &Submesh) -> Option<&[u32]> {
        let end = submesh.first_index.checked_add(submesh.index_count)?;
        self.indices
            .get(submesh.first_index..end)
            .filter(|indices| !self.positions.is_empty() && !indices.is_empty())
    }
}
