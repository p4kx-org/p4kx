// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{Mesh, Submesh};

use super::{Layout, PrimitiveLayout};
use crate::block_data::BlockData;
use crate::materials::MaterialPlan;

pub(super) struct DrawableSubmesh<'m> {
    ordinal: usize,
    pub(super) source: &'m Submesh,
    indices: &'m [u32],
}

impl<'m> Layout<'m> {
    pub(super) fn drawable_submeshes(
        &mut self,
        mesh: &'m Mesh,
        materials: &MaterialPlan<'_>,
    ) -> Vec<DrawableSubmesh<'m>> {
        let mut drawn = Vec::new();
        for (ordinal, submesh) in mesh.submeshes.iter().enumerate() {
            let Some(indices) = mesh.submesh_indices(submesh) else {
                continue;
            };
            if materials.hidden(submesh.material_id) {
                self.hidden_primitives += 1;
            } else {
                drawn.push(DrawableSubmesh {
                    ordinal,
                    source: submesh,
                    indices,
                });
            }
        }
        drawn
    }

    pub(super) fn plan_primitives(
        &mut self,
        primitives: &[DrawableSubmesh<'m>],
        reading: &[bool],
        vertices: usize,
    ) -> Vec<PrimitiveLayout> {
        let short = u16::try_from(vertices).is_ok();
        primitives
            .iter()
            .zip(reading)
            .map(|(drawn, &colours)| PrimitiveLayout {
                indices: self.push(if short {
                    BlockData::Short(drawn.indices)
                } else {
                    BlockData::Wide(drawn.indices)
                }),
                material_id: drawn.source.material_id,
                model_submesh: drawn.ordinal,
                colours,
            })
            .collect()
    }
}
