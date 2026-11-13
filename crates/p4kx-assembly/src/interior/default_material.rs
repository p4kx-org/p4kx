// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;
use std::sync::Arc;

use p4kx_extraction::ArchiveDefaultMaterial;
use p4kx_gltf::{
    GlbOptions, GlbSummary, GltfError, Scene, SceneNode, ScenePart, TextureLoader, write_scene_glb,
    write_scene_glb_with,
};
use p4kx_model::{MaterialStatus, Model, Transform};

use super::{Interior, InteriorPart};
use crate::ModelProvider;

fn parts_models(parts: &[InteriorPart]) -> impl Iterator<Item = (&Model, &MaterialStatus)> {
    parts
        .iter()
        .filter_map(|part| Some((part.model()?, part.materials())))
}

pub(super) fn resolve(
    parts: &[InteriorPart],
    provider: &dyn ModelProvider,
) -> Option<Arc<ArchiveDefaultMaterial>> {
    let requires_fallback = parts_models(parts).any(|(model, materials)| {
        model
            .lods
            .iter()
            .any(|lod| lod.requires_material_fallback(materials))
    });
    if requires_fallback {
        provider.default_material()
    } else {
        None
    }
}

impl Interior {
    /// Returns the original fallback decision for the Interior's drawn models at this LOD.
    #[must_use]
    pub fn default_material(&self, level: u32) -> Option<&ArchiveDefaultMaterial> {
        self.default_material.as_deref().filter(|_| {
            parts_models(&self.parts).any(|(model, materials)| {
                model
                    .nearest_lod(level)
                    .is_some_and(|lod| lod.requires_material_fallback(materials))
            })
        })
    }

    /// Adds the parts to `scene`, the root container below the node `parent` placed by
    /// `placement`, every other part below its own parent; returns the index of the root's node,
    /// or `None` when the Interior has no parts and nothing was added.
    ///
    /// # Errors
    ///
    /// Returns [`GltfError::InvalidSceneNode`] when `parent` is not a node of `scene`.
    pub fn add_to_scene<'s>(
        &'s self,
        scene: &mut Scene<'s>,
        parent: Option<usize>,
        placement: Transform,
    ) -> Result<Option<usize>, GltfError> {
        let mut nodes = Vec::with_capacity(self.parts.len());
        let fallback = self
            .default_material
            .as_deref()
            .and_then(ArchiveDefaultMaterial::material);
        for part in &self.parts {
            let (above, local) = part.parent.map_or((parent, placement), |index| {
                (nodes.get(index).copied(), part.local)
            });
            let mut node = SceneNode::new(part.name.clone(), above, local);
            if let Some(model) = &part.model {
                let scene_part = ScenePart::new(model, &part.materials);
                let mut scene_part =
                    fallback.map_or(scene_part, |material| scene_part.with_fallback(material));
                if let Some(palette) = &part.palette {
                    scene_part = scene_part.with_palette(palette);
                }
                node = node.with_part(scene_part);
            }
            if let Some(light) = part.scene_light() {
                node = node.with_light(light);
            }
            nodes.push(scene.push(node)?);
        }
        Ok(nodes.first().copied())
    }

    /// Writes the Interior as a GLB file to `out`: a node per part below the node of its parent,
    /// placed by its transform, with the nodes of its model below it; `out` should be buffered.
    /// `options.lod` should be the level the Interior was built at.
    ///
    /// # Errors
    ///
    /// Returns [`GltfError::TooLarge`] when the Interior does not fit in a GLB file and
    /// [`GltfError::Write`] when `out` fails.
    pub fn write_glb(
        &self,
        options: &GlbOptions,
        out: &mut impl Write,
    ) -> Result<GlbSummary, GltfError> {
        let mut scene = Scene::new();
        self.add_to_scene(&mut scene, None, Transform::IDENTITY)?;
        write_scene_glb(&scene, options, out)
    }

    /// Writes the Interior as a GLB file to `out` like [`Self::write_glb`], taking the textures
    /// of [`MaterialMode::Textures`](p4kx_gltf::MaterialMode::Textures) from `textures`; a texture
    /// that cannot be loaded leaves the material's plain colour and is listed in
    /// [`GlbSummary::texture_fallbacks`].
    ///
    /// # Errors
    ///
    /// As [`Self::write_glb`].
    pub fn write_glb_with(
        &self,
        options: &GlbOptions,
        textures: &dyn TextureLoader,
        out: &mut impl Write,
    ) -> Result<GlbSummary, GltfError> {
        let mut scene = Scene::new();
        self.add_to_scene(&mut scene, None, Transform::IDENTITY)?;
        write_scene_glb_with(&scene, options, textures, out)
    }
}
