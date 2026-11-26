// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_extraction::model::MeshOrigin;
use p4kx_extraction::{FrozenGlbMeshOrigins, GlbMeshOrigins};
use p4kx_gltf::{
    GlbOptions, GlbSummary, GltfError, MeshVertexMap, RigAnimation, RiggedGlbEmission,
    TextureLoader, write_rigged_glb_animated_observed,
    write_rigged_glb_animated_observed_with_vertex_maps, write_rigged_glb_with_vertex_maps,
};

use super::{Armour, ArmourPart, CharacterAnimations, RigArchiveRead};

impl CharacterAnimations<'_> {
    /// Returns the original input positions of animations passed to the GLB writer, in its order.
    pub fn input_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.keys
            .iter()
            .map(|(index, _, _)| *index)
            .chain(self.sampled_indices.iter().copied())
    }

    /// Returns the rig read used by a sampled animation in writer order.
    #[must_use]
    pub fn rig_read_for(&self, animation: usize) -> Option<&RigArchiveRead> {
        let sampled = animation.checked_sub(self.keys.len())?;
        self.sampled_rigs.get(sampled)?.as_ref()
    }

    pub(super) fn rig_animations(&self) -> Vec<RigAnimation<'_>> {
        self.keys
            .iter()
            .map(|(_, animation, driven)| RigAnimation::Keys(animation, driven))
            .chain(self.sampled.iter().map(RigAnimation::Sampled))
            .collect()
    }
}

struct EmittedMesh {
    accessor: usize,
    source: Option<MeshOrigin>,
    part: usize,
}

impl Armour {
    /// Writes the rigged GLB and retains observed geometry and worn mount field origins.
    ///
    /// # Errors
    /// Returns a GLB write or source evidence error.
    pub fn write_glb_with_origins(
        &self,
        animations: &CharacterAnimations<'_>,
        options: &GlbOptions,
        textures: &dyn TextureLoader,
        out: &mut impl Write,
    ) -> Result<(GlbSummary, FrozenGlbMeshOrigins), GltfError> {
        let mut meshes = Vec::new();
        let (summary, emission) = self.write_glb_animated_observed_with_vertex_origins(
            animations,
            options,
            textures,
            out,
            &mut |mesh, _| {
                meshes.push(EmittedMesh {
                    accessor: mesh.position_accessor,
                    source: mesh.origin,
                    part: mesh.part,
                });
                Ok(())
            },
        )?;
        let mut origins = GlbMeshOrigins::with_expanded_use_refs();
        // Worn fields register first so their original reference encoding is stable.
        let mounts = self
            .worn_mount_fields(&emission, origins.source_uses())
            .map_err(io::Error::other)?;
        origins.include_json_fields(mounts);
        for mesh in meshes {
            let part = self
                .parts()
                .get(mesh.part)
                .ok_or_else(|| io::Error::other("rigged mesh lost its producing part"))?;
            origins.observe_mesh(mesh.accessor, mesh.source, part.model_archive_sources())?;
        }
        Ok((summary, origins.finish()?))
    }

    /// Writes the rigged GLB and visits output vertex mappings with their exact producing part.
    ///
    /// # Errors
    /// Returns ordinary rigged write failures or an error from the visitor.
    pub fn write_glb_with_vertex_origins(
        &self,
        animations: &CharacterAnimations<'_>,
        options: &GlbOptions,
        textures: &dyn TextureLoader,
        out: &mut impl Write,
        visitor: &mut impl FnMut(MeshVertexMap<'_>, &ArmourPart) -> io::Result<()>,
    ) -> Result<GlbSummary, GltfError> {
        let pieces = self.rig_pieces();
        write_rigged_glb_with_vertex_maps(
            &self.skeleton.bones,
            &pieces,
            &animations.rig_animations(),
            options,
            textures,
            out,
            &mut vertex_origin_visitor(&self.parts, visitor),
        )
    }

    /// Writes the rigged GLB with the emitted geometry and material slots of each part.
    ///
    /// # Errors
    /// As [`Self::write_glb_animated_with`].
    pub fn write_glb_animated_observed(
        &self,
        animations: &CharacterAnimations<'_>,
        options: &GlbOptions,
        textures: &dyn TextureLoader,
        out: &mut impl Write,
    ) -> Result<(GlbSummary, RiggedGlbEmission), GltfError> {
        let pieces = self.rig_pieces();
        write_rigged_glb_animated_observed(
            &self.skeleton.bones,
            &pieces,
            &animations.rig_animations(),
            options,
            textures,
            out,
        )
    }

    /// Writes the rigged GLB with emission evidence and visits each output vertex mapping.
    ///
    /// # Errors
    /// Returns ordinary rigged write failures or an error from the visitor.
    pub fn write_glb_animated_observed_with_vertex_origins(
        &self,
        animations: &CharacterAnimations<'_>,
        options: &GlbOptions,
        textures: &dyn TextureLoader,
        out: &mut impl Write,
        visitor: &mut impl FnMut(MeshVertexMap<'_>, &ArmourPart) -> io::Result<()>,
    ) -> Result<(GlbSummary, RiggedGlbEmission), GltfError> {
        let pieces = self.rig_pieces();
        write_rigged_glb_animated_observed_with_vertex_maps(
            &self.skeleton.bones,
            &pieces,
            &animations.rig_animations(),
            options,
            textures,
            out,
            &mut vertex_origin_visitor(&self.parts, visitor),
        )
    }
}

fn vertex_origin_visitor<'armour>(
    parts: &'armour [ArmourPart],
    visitor: &'armour mut impl FnMut(MeshVertexMap<'_>, &ArmourPart) -> io::Result<()>,
) -> impl FnMut(MeshVertexMap<'_>) -> io::Result<()> + 'armour {
    move |map| {
        let part = parts
            .get(map.part)
            .ok_or_else(|| io::Error::other("rigged mesh lost its producing part"))?;
        visitor(map, part)
    }
}
