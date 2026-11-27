// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::model::{CUBE_CAPS_MATERIAL, TestSubmesh, TestVertex};
use crate::{TestModel, TestPositions};

impl TestModel {
    /// Stores the extra words between the mesh descriptors and streams.
    #[must_use]
    pub fn with_mesh_extra_words(mut self, words: Vec<u32>) -> Self {
        self.mesh_extra_words = words;
        self
    }

    /// Returns a skin without nodes with one triangle per place, each its own submesh numbered by
    /// its position as the node and drawn with material ID [`CUBE_CAPS_MATERIAL`]: the shape of
    /// a body skin, whose submeshes are its zones. A place is the `[x, z]` of the triangle's
    /// corner, in metres.
    #[must_use]
    pub fn zoned(places: &[[f32; 2]]) -> Self {
        let zones = places.len();
        let mut model = Self::cube()
            .with_positions(TestPositions::Float)
            .with_nodes(Vec::new());
        let at = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
        model.vertices = places
            .iter()
            .flat_map(|&[x, z]| {
                [[x, z], [x + 0.05, z], [x, z + 0.05]]
                    .into_iter()
                    .map(|[px, pz]| TestVertex {
                        position: [px, 0.0, pz],
                        normal: [0.0, -1.0, 0.0],
                        uv: [0.0, 0.0],
                    })
            })
            .collect();
        model.indices = (0..at(zones.saturating_mul(3))).collect();
        model.submeshes = (0..zones)
            .map(|zone| TestSubmesh {
                material_id: CUBE_CAPS_MATERIAL,
                node: u16::try_from(zone).unwrap_or(u16::MAX),
                first_index: at(zone * 3),
                index_count: 3,
                first_vertex: at(zone * 3),
                vertex_count: 3,
            })
            .collect();
        model
    }
}
