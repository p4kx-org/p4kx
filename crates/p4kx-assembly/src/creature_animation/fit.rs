// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::io::Write;

use p4kx_gltf::{
    GlbOptions, GlbSummary, GltfError, TextureLoader, write_scene_glb, write_scene_glb_with,
};
use p4kx_model::{Animation, Bone};

use crate::animation_budget::{KeyFit, ValueBudget};
use crate::armour::MAX_ANIMATION_VALUES;
use crate::armour::unused::summarise;
use crate::{Assembly, Issue, IssueKind, creature_rig};

/// Why an additive clip is left out: glTF cannot say that its keys are offsets.
const ADDITIVE: &str = "additive animation, its keys are offsets that need a base animation";
/// Why a clip that moves nothing is left out.
const NO_BONE: &str = "no track names a bone of the skeleton";
/// Why every clip is left out of an Assembly that writes no skeleton.
const NO_SKELETON: &str = "the Assembly has no creature skeleton to animate";

/// The animations of a creature Assembly that fit its skeleton, ready to be written with it by
/// [`Assembly::write_glb_animated_with`]. Made by [`Assembly::animate`].
#[derive(Debug, Clone, Default)]
pub struct CreatureAnimations<'a> {
    fitted: Vec<&'a Animation>,
    issues: Vec<Issue>,
    values: usize,
}

impl<'a> CreatureAnimations<'a> {
    /// Returns the number of animations that will be written.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.fitted.len()
    }

    /// Returns whether no animation will be written.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.fitted.is_empty()
    }

    /// Returns the animations that will be written.
    #[must_use]
    pub fn animations(&self) -> &[&'a Animation] {
        &self.fitted
    }

    /// Returns the animations left out as [`IssueKind::AnimationNotUsed`], one issue per reason,
    /// named `animations` when it sums up several.
    #[must_use]
    pub fn issues(&self) -> &[Issue] {
        &self.issues
    }

    /// Returns the number of values the animations write, counted against the budget.
    #[must_use]
    pub const fn values(&self) -> usize {
        self.values
    }
}

fn not_used(animation: &Animation, reason: &str) -> Issue {
    Issue {
        path: animation.path.clone(),
        name: animation.name.clone(),
        kind: IssueKind::AnimationNotUsed {
            reason: reason.to_owned(),
            count: 1,
            examples: Vec::new(),
        },
    }
}

/// Returns the values `animation` costs when it is written on the bones of `skeleton` within
/// `budget`, or why it is not.
fn left_out(
    skeleton: Option<&[Bone]>,
    animation: &Animation,
    budget: &ValueBudget,
) -> Result<usize, Cow<'static, str>> {
    let bones = skeleton.ok_or(Cow::Borrowed(NO_SKELETON))?;
    if animation.additive {
        return Err(Cow::Borrowed(ADDITIVE));
    }
    match budget.fit_keys(animation, !animation.bind(bones).bound.is_empty()) {
        KeyFit::Fits(cost) => Ok(cost),
        KeyFit::NoBone => Err(Cow::Borrowed(NO_BONE)),
        KeyFit::OverBudget(reason) => Err(Cow::Owned(reason)),
    }
}

impl Assembly {
    /// Fits `animations` to the skeleton of the creature, within [`MAX_ANIMATION_VALUES`].
    ///
    /// An animation is left out, and listed in [`CreatureAnimations::issues`], when it is
    /// additive, when no track names a bone of the skeleton, and, with a reason starting with
    /// `limit`, when it would exceed the budget; the animations before it are kept.
    #[must_use]
    pub fn animate<'a>(&self, animations: &'a [Animation]) -> CreatureAnimations<'a> {
        self.animate_within(animations, MAX_ANIMATION_VALUES)
    }

    /// Fits `animations` like [`Assembly::animate`], with a budget of `budget` values.
    #[must_use]
    pub fn animate_within<'a>(
        &self,
        animations: &'a [Animation],
        budget: usize,
    ) -> CreatureAnimations<'a> {
        let mut fit = CreatureAnimations::default();
        let skeleton = creature_rig::skeleton_of(&self.parts, &self.issues, self.lod);
        let mut left = ValueBudget::new(budget);
        for animation in animations {
            match left_out(skeleton, animation, &left) {
                Ok(cost) => {
                    left.spend(cost);
                    fit.fitted.push(animation);
                }
                Err(reason) => fit.issues.push(not_used(animation, &reason)),
            }
        }
        fit.values = left.spent();
        fit.issues = summarise(fit.issues, budget);
        fit
    }

    /// Writes the Assembly with `animations` as a GLB file to `out`, like
    /// [`Assembly::write_glb`], and the animations as glTF animations of the bone nodes.
    ///
    /// # Errors
    ///
    /// As [`Assembly::write_glb`].
    pub fn write_glb_animated(
        &self,
        animations: &CreatureAnimations<'_>,
        options: &GlbOptions,
        out: &mut impl Write,
    ) -> Result<GlbSummary, GltfError> {
        let scene = self
            .scene()?
            .with_animations(animations.fitted.iter().copied());
        write_scene_glb(&scene, options, out)
    }

    /// Writes the Assembly with `animations` like [`Assembly::write_glb_animated`], taking the
    /// textures of [`MaterialMode::Textures`](p4kx_gltf::MaterialMode::Textures) from `textures`.
    ///
    /// # Errors
    ///
    /// As [`Assembly::write_glb`].
    pub fn write_glb_animated_with(
        &self,
        animations: &CreatureAnimations<'_>,
        options: &GlbOptions,
        textures: &dyn TextureLoader,
        out: &mut impl Write,
    ) -> Result<GlbSummary, GltfError> {
        let scene = self
            .scene()?
            .with_animations(animations.fitted.iter().copied());
        write_scene_glb_with(&scene, options, textures, out)
    }
}
