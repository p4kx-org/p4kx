// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::ArchiveDefaultMaterial;
use p4kx_model::{MaterialStatus, Model};

use super::{Assembly, Part};
use crate::ModelProvider;

fn parts_models(parts: &[Part]) -> impl Iterator<Item = (&Model, &MaterialStatus)> {
    parts.iter().flat_map(|part| {
        part.model()
            .map(|model| (model, part.materials()))
            .into_iter()
            .chain(
                part.attachments()
                    .iter()
                    .map(|attachment| (attachment.model(), attachment.materials())),
            )
    })
}

impl Assembly {
    pub(crate) fn with_default_material(mut self, models: &dyn ModelProvider) -> Self {
        if parts_models(&self.parts).any(|(model, status)| {
            model
                .lods
                .iter()
                .any(|lod| lod.requires_material_fallback(status))
        }) {
            self.default_material = models.default_material();
        }
        self
    }

    /// Returns the observed fallback for the Assembly and its Interior at this output LOD.
    #[must_use]
    pub fn default_material(&self, lod: u32) -> Option<&ArchiveDefaultMaterial> {
        self.default_material
            .as_deref()
            .filter(|_| {
                parts_models(&self.parts).any(|(model, status)| {
                    model
                        .nearest_lod(lod)
                        .is_some_and(|lod| lod.requires_material_fallback(status))
                })
            })
            .or_else(|| {
                self.interior()?
                    .containers()
                    .iter()
                    .find_map(|container| container.interior().default_material(lod))
            })
    }
}
