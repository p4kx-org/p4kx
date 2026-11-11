// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use p4kx_gltf::{
    GlbOptions, GlbSummary, GltfError, RigAnimation, RiggedGlbEmission, TextureLoader,
    write_rigged_glb_animated_observed,
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

impl Armour {
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
        let pieces: Vec<_> = self.parts.iter().map(ArmourPart::rig_piece).collect();
        write_rigged_glb_animated_observed(
            &self.skeleton.bones,
            &pieces,
            &animations.rig_animations(),
            options,
            textures,
            out,
        )
    }
}
