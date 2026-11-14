// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::{ArchiveDefaultMaterial, ModelArchiveSource};
use p4kx_gltf::{
    GlbOptions, GlbSummary, GltfError, MeshVertexMap, Scene, SceneNode, ScenePart, TextureLoader,
    write_scene_glb_observed,
};
use p4kx_model::Model;

use crate::{Assembly, creature_rig};

impl Assembly {
    pub(crate) fn scene(&self) -> Result<Scene<'_>, GltfError> {
        self.scene_with(|_, _| {})
    }

    /// Writes an Assembly GLB and visits emitted meshes with their original part or attachment sources.
    ///
    /// # Errors
    /// Returns scene and write failures, or an error from the visitor.
    pub fn write_glb_with_vertex_origins(
        &self,
        options: &GlbOptions,
        textures: &dyn TextureLoader,
        out: &mut impl io::Write,
        visitor: &mut impl FnMut(MeshVertexMap<'_>, &Model, &[ModelArchiveSource]) -> io::Result<()>,
    ) -> Result<GlbSummary, GltfError> {
        let mut sources: Vec<&[ModelArchiveSource]> = Vec::new();
        let scene = self.scene_with(|node, origin| {
            sources.resize(node + 1, &[]);
            if let Some(slot) = sources.get_mut(node) {
                *slot = origin;
            }
        })?;
        write_scene_glb_observed(&scene, options, Some(textures), out, &mut |map| {
            let owned = map.scene_node.and_then(|node| {
                let scene_node = scene.nodes().get(node)?;
                let model = scene_node.part?.model;
                Some((model, *sources.get(node)?))
            });
            let (model, sources) =
                owned.ok_or_else(|| io::Error::other("scene mesh lost its producer node"))?;
            visitor(map, model, sources)
        })
    }

    fn scene_with<'a>(
        &'a self,
        mut observe: impl FnMut(usize, &'a [ModelArchiveSource]),
    ) -> Result<Scene<'a>, GltfError> {
        let mut scene = creature_rig::skeleton_of(&self.parts, &self.issues, self.lod)
            .map_or_else(Scene::new, |skeleton| Scene::new().with_skeleton(skeleton));
        let fallback = self
            .default_material
            .as_deref()
            .and_then(ArchiveDefaultMaterial::material);
        for part in &self.parts {
            let mut node = SceneNode::new(part.name.clone(), part.parent, part.local);
            if let Some(model) = &part.model {
                let drawn = ScenePart::new(model, part.materials());
                let drawn = fallback.map_or(drawn, |material| drawn.with_fallback(material));
                node = node.with_part(
                    part.palette()
                        .map_or(drawn, |palette| drawn.with_palette(palette)),
                );
            }
            let index = scene.push(node)?;
            observe(index, part.model_archive_sources());
        }
        for (index, attached) in self.attached_models() {
            let drawn = ScenePart::new(&attached.model, attached.materials());
            let drawn = fallback.map_or(drawn, |material| drawn.with_fallback(material));
            let palette = self.parts.get(index).and_then(crate::Part::palette);
            let node = SceneNode::new(attached.name.clone(), Some(index), attached.local)
                .with_part(palette.map_or(drawn, |palette| drawn.with_palette(palette)));
            let index = scene.push(node)?;
            observe(index, attached.model_archive_sources());
        }
        if let Some(interior) = &self.interior {
            interior.add_to_scene(&mut scene, self.parts.len(), &mut observe)?;
        }
        Ok(scene)
    }
}
