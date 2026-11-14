// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::ArchiveDefaultMaterial;
use p4kx_gltf::RigPiece;

use super::Armour;
use crate::ModelProvider;

impl Armour {
    pub(super) fn with_default_material(mut self, provider: &dyn ModelProvider) -> Self {
        if self.parts.iter().any(|part| {
            part.model()
                .lods
                .iter()
                .any(|lod| lod.requires_material_fallback(part.materials()))
        }) {
            self.default_material = provider.default_material();
        }
        self
    }

    /// Returns the observed physical fallback for the rig at the selected output LOD.
    #[must_use]
    pub fn default_material(&self, lod: u32) -> Option<&ArchiveDefaultMaterial> {
        self.default_material.as_deref().filter(|_| {
            self.parts.iter().any(|part| {
                part.model()
                    .nearest_lod(lod)
                    .is_some_and(|lod| lod.requires_material_fallback(part.materials()))
            })
        })
    }

    /// Returns the physical source when a verified fallback material is available.
    #[must_use]
    pub fn resolved_default_material(&self) -> Option<&ArchiveDefaultMaterial> {
        self.default_material
            .as_deref()
            .filter(|fallback| fallback.material().is_some())
    }

    pub(super) fn rig_pieces(&self) -> Vec<RigPiece<'_>> {
        let material = self
            .default_material
            .as_deref()
            .and_then(ArchiveDefaultMaterial::material);
        self.parts
            .iter()
            .map(|part| {
                let piece = part.rig_piece();
                material.map_or(piece, |material| piece.with_fallback(material))
            })
            .collect()
    }
}
