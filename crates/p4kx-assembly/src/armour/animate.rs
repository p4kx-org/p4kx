// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::io::Write;

use p4kx_character::{Dna, DnaRig, FaceRig, RigLogic};
use p4kx_gltf::{
    GlbOptions, GlbSummary, GltfError, RigAnimation, RigPiece, SampledAnimation, SampledWeights,
    TextureLoader, write_rigged_glb_animated, write_rigged_glb_animated_with,
};
use p4kx_model::Animation;

use super::facial::{FaceLink, FaceSource};
use super::sample::{Control, evaluate, joint_samples, reduced, values_of, weight_samples};
use super::shape::MAX_DNA_RIG_LEN;
use super::unused::summarise;
use super::{Armour, ArmourPart};
use crate::animation_budget::{key_cost, limit_reason};
use crate::{Issue, IssueKind, ModelFailure, ModelProvider};

/// The most values, floats of key times and poses, the animations of one Character may write: 32
/// Mi values, 128 MiB. The 141 facial animations of one head together hold about 150 Mi.
pub const MAX_ANIMATION_VALUES: usize = 1 << 25;

/// The most distinct sample times of one facial animation.
const MAX_SAMPLES: usize = 1 << 15;

/// The animations of a Character ready to be written with its [`Armour`].
///
/// They are the animations that move the bones of the body by their keys, and the facial
/// animations evaluated through the rig logic into joint poses and morph weights. Made by
/// [`Armour::animate`].
#[derive(Debug, Clone, Default)]
pub struct CharacterAnimations<'a> {
    keys: Vec<(&'a Animation, Vec<u32>)>,
    sampled: Vec<SampledAnimation>,
    issues: Vec<Issue>,
    values: usize,
}

impl CharacterAnimations<'_> {
    /// Returns the number of animations that will be written: those with keys for bones and the
    /// facial ones; an animation of both kinds counts twice.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.keys.len() + self.sampled.len()
    }

    /// Returns whether no animation will be written.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns what could not be done: animations that fit nothing of the Character, that would
    /// exceed the budget, and facial animations whose rig cannot be read. Animations left out for
    /// the same reason are one `animation_not_used` issue with their count and a few names.
    #[must_use]
    pub fn issues(&self) -> &[Issue] {
        &self.issues
    }

    /// Returns the facial animations, evaluated at their key frames.
    #[must_use]
    pub fn sampled(&self) -> &[SampledAnimation] {
        &self.sampled
    }

    /// Returns the number of values the animations write, counted against the budget.
    #[must_use]
    pub const fn values(&self) -> usize {
        self.values
    }
}

/// The rig logic of one head, loaded once for all animations.
struct Loaded {
    face: FaceRig,
}

/// A facial animation evaluated, with the controller IDs of its controls and its cost.
struct Fitted {
    sampled: SampledAnimation,
    driven: Vec<u32>,
    cost: usize,
}

/// Why an animation has no facial part.
enum Refusal {
    Limit(String),
    Failed(String),
}

fn issue(animation: &Animation, kind: IssueKind) -> Issue {
    Issue {
        path: animation.path.clone(),
        name: animation.name.clone(),
        kind,
    }
}

fn not_used(animation: &Animation, reason: String) -> Issue {
    issue(
        animation,
        IssueKind::AnimationNotUsed {
            reason,
            count: 1,
            examples: Vec::new(),
        },
    )
}

fn limit(values: usize, remaining: usize) -> String {
    limit_reason(values, remaining)
}

/// The Issue for an animation the facial part of which was refused, or that nothing of fits;
/// none when the bone part has been reported already.
fn refused(
    animation: &Animation,
    refusal: Option<Refusal>,
    (used, reported): (bool, bool),
    has_rig: bool,
) -> Option<Issue> {
    match refusal {
        _ if reported => None,
        Some(Refusal::Limit(reason)) => Some(not_used(animation, reason)),
        Some(Refusal::Failed(reason)) if !used => {
            Some(issue(animation, IssueKind::FaceAnimationFailed { reason }))
        }
        _ if used => None,
        _ => {
            let reason = if has_rig {
                "no track names a bone of the skeleton or a control of the face rig"
            } else {
                "no track names a bone of the skeleton, and the Character has no head rig"
            };
            Some(not_used(animation, reason.to_owned()))
        }
    }
}

fn load(link: &FaceLink, models: &dyn ModelProvider) -> Result<Loaded, String> {
    let face = match &link.source {
        FaceSource::Dna { rig, dna } => dna_face(rig, dna, &link.logic, models)?,
        FaceSource::Own => FaceRig::from_logic(&link.logic).map_err(|error| error.to_string())?,
    };
    Ok(Loaded { face })
}

/// Makes the face rig of a head shaped by `dna` from the DNA rig at `rig_path` and the rig logic
/// `logic` of its skin.
fn dna_face(
    rig_path: &str,
    dna: &Dna,
    logic: &RigLogic,
    models: &dyn ModelProvider,
) -> Result<FaceRig, String> {
    let bytes = models
        .file(rig_path, MAX_DNA_RIG_LEN)
        .map_err(|failure| match failure {
            ModelFailure::Missing => format!("the DNA rig {rig_path} does not exist"),
            other => format!("the DNA rig {rig_path} cannot be read: {other}"),
        })?;
    let rig = DnaRig::read(&bytes).map_err(|error| format!("{rig_path}: {error}"))?;
    let matrix = rig
        .matrix(dna)
        .map_err(|error| format!("{rig_path}: {error}"))?;
    FaceRig::new(&rig, &matrix, logic).map_err(|error| format!("{rig_path}: {error}"))
}

impl Armour {
    /// Returns the part with the rig that animates the face, and its index among the parts.
    fn face_part(&self) -> Option<(usize, &ArmourPart, &FaceLink)> {
        self.parts
            .iter()
            .enumerate()
            .find_map(|(index, part)| Some((index, part, part.face.as_deref()?)))
    }

    /// Fits `animations` to the Character, within [`MAX_ANIMATION_VALUES`].
    ///
    /// Tracks that name a bone of the skeleton move it by their keys. Tracks that name a control
    /// of the head rig are evaluated through the rig logic at the frames where a control has a key:
    /// the joints of the face move to the poses the rig gives and the morph targets of the head
    /// take the weights of their blendshapes; this needs the head to have been built from a DNA at
    /// level of detail 0. Control values are held to 0 to 1. An animation that fits nothing, one
    /// that would exceed the budget (listed as `animation_not_used` with a reason starting with
    /// `limit`), and a facial animation whose rig cannot be read or that gives a pose that is not
    /// finite is listed in [`CharacterAnimations::issues`]; tracks that fit neither are left out
    /// and listed in the summary of the GLB.
    #[must_use]
    pub fn animate<'a>(
        &self,
        animations: &'a [Animation],
        models: &dyn ModelProvider,
    ) -> CharacterAnimations<'a> {
        self.animate_within(animations, models, MAX_ANIMATION_VALUES)
    }

    /// Fits `animations` like [`Armour::animate`], with a budget of `budget` values.
    #[must_use]
    pub fn animate_within<'a>(
        &self,
        animations: &'a [Animation],
        models: &dyn ModelProvider,
        budget: usize,
    ) -> CharacterAnimations<'a> {
        let mut out = CharacterAnimations::default();
        let mut loaded: Option<Result<Loaded, String>> = None;
        let mut remaining = budget;
        for animation in animations {
            let bound = animation.bind(&self.skeleton.bones);
            let ids: HashSet<u32> = bound
                .bound
                .iter()
                .map(|(_, track)| track.controller_id)
                .collect();
            let fitted = if bound.unbound.is_empty() {
                Ok(None)
            } else {
                self.face_fit(animation, (&ids, remaining), (&mut loaded, models))
            };
            let (mut driven, mut sampled, mut refusal) = (Vec::new(), None, None);
            match fitted {
                Ok(Some(fitted)) => {
                    remaining = remaining.saturating_sub(fitted.cost);
                    out.values += fitted.cost;
                    driven = fitted.driven;
                    sampled = Some(fitted.sampled);
                }
                Ok(None) => {}
                Err(failed) => refusal = Some(failed),
            }
            let mut used = sampled.is_some();
            if let Some(sampled) = sampled {
                out.sampled.push(sampled);
            }
            let cost = key_cost(animation);
            let mut reported = false;
            if !bound.bound.is_empty() {
                if cost <= remaining {
                    remaining -= cost;
                    out.values += cost;
                    out.keys.push((animation, driven));
                    used = true;
                } else {
                    out.issues.push(not_used(animation, limit(cost, remaining)));
                    reported = true;
                }
            }
            let has_rig = self.face_part().is_some();
            out.issues
                .extend(refused(animation, refusal, (used, reported), has_rig));
        }
        out.issues = summarise(out.issues, budget);
        out
    }

    /// Evaluates the facial part of `animation`, loading the rig on the first use.
    fn face_fit(
        &self,
        animation: &Animation,
        (ids, remaining): (&HashSet<u32>, usize),
        (loaded, models): (&mut Option<Result<Loaded, String>>, &dyn ModelProvider),
    ) -> Result<Option<Fitted>, Refusal> {
        let Some((part, _, link)) = self.face_part() else {
            return Ok(None);
        };
        match loaded.get_or_insert_with(|| load(link, models)) {
            Ok(rig) => self.sample(animation, (ids, remaining), rig, (part, link)),
            Err(reason) => Err(Refusal::Failed(reason.clone())),
        }
    }

    fn sample(
        &self,
        animation: &Animation,
        (bones, remaining): (&HashSet<u32>, usize),
        rig: &Loaded,
        (part, link): (usize, &FaceLink),
    ) -> Result<Option<Fitted>, Refusal> {
        let logic = rig.face.logic();
        let mut controls: Vec<Control<'_>> = Vec::new();
        let mut driven = Vec::new();
        let mut skipped = Vec::new();
        for track in &animation.tracks {
            match (logic.control_by_hash(track.controller_id), values_of(track)) {
                (Some(control), Some((frames, values))) => {
                    driven.push(track.controller_id);
                    if control != 0 {
                        controls.push((control, frames, values));
                    }
                }
                (None, _) if bones.contains(&track.controller_id) => {}
                _ => skipped.push(track.controller_id),
            }
        }
        if controls.is_empty() {
            return Ok(None);
        }
        let mut frames: Vec<u16> = controls
            .iter()
            .flat_map(|(_, frames, _)| frames.iter().copied())
            .collect();
        frames.sort_unstable();
        frames.dedup();
        let targets = self.weighed(part, link);
        let weighed: usize = targets.iter().map(|(_, numbers)| numbers.len()).sum();
        let estimate = frames
            .len()
            .saturating_mul(rig.face.joints() * 7 + weighed + 1);
        if frames.len() > MAX_SAMPLES || estimate > remaining {
            return Err(Refusal::Limit(limit(estimate, remaining)));
        }
        let poses = evaluate(&rig.face, &controls, &frames)
            .ok_or_else(|| Refusal::Failed("the rig gives a pose that is not finite".to_owned()))?;
        let rate = f32::from(animation.frames_per_second.max(1));
        let (times, poses) = reduced((&frames, &poses), link, rate);
        let hashes = (logic.joint_hashes(), &mut skipped);
        let moved = joint_samples(&self.skeleton.bones, &poses, link, hashes);
        let weights: Vec<SampledWeights> = targets
            .iter()
            .flat_map(|(at, numbers)| weight_samples(&poses, numbers, *at))
            .collect();
        let held: usize = weights.iter().map(|run| run.values.len()).sum();
        let cost = times.len() * (1 + moved.len() * 7) + held;
        Ok(Some(Fitted {
            sampled: SampledAnimation::new(animation.name.clone(), times, moved, weights, skipped),
            driven,
            cost,
        }))
    }

    /// Writes the Armour with `animations` as one GLB file to `out`, see [`Armour::write_glb`].
    ///
    /// # Errors
    ///
    /// Returns [`GltfError::TooLarge`] when the Armour does not fit in a GLB file and
    /// [`GltfError::Write`] when `out` fails.
    pub fn write_glb_animated(
        &self,
        animations: &CharacterAnimations<'_>,
        options: &GlbOptions,
        out: &mut impl Write,
    ) -> Result<GlbSummary, GltfError> {
        self.write_animated(animations, options, None, out)
    }

    /// Writes the Armour with `animations` like [`Armour::write_glb_animated`], taking the
    /// textures of [`MaterialMode::Textures`](p4kx_gltf::MaterialMode::Textures) from `textures`.
    ///
    /// # Errors
    ///
    /// As [`Armour::write_glb_animated`].
    pub fn write_glb_animated_with(
        &self,
        animations: &CharacterAnimations<'_>,
        options: &GlbOptions,
        textures: &dyn TextureLoader,
        out: &mut impl Write,
    ) -> Result<GlbSummary, GltfError> {
        self.write_animated(animations, options, Some(textures), out)
    }

    fn write_animated(
        &self,
        animations: &CharacterAnimations<'_>,
        options: &GlbOptions,
        textures: Option<&dyn TextureLoader>,
        out: &mut impl Write,
    ) -> Result<GlbSummary, GltfError> {
        let pieces: Vec<RigPiece<'_>> = self.parts.iter().map(ArmourPart::rig_piece).collect();
        let list: Vec<RigAnimation<'_>> = animations
            .keys
            .iter()
            .map(|(animation, driven)| RigAnimation::Keys(animation, driven))
            .chain(animations.sampled.iter().map(RigAnimation::Sampled))
            .collect();
        let bones = &self.skeleton.bones;
        match textures {
            Some(textures) => {
                write_rigged_glb_animated_with(bones, &pieces, &list, options, textures, out)
            }
            None => write_rigged_glb_animated(bones, &pieces, &list, options, out),
        }
    }
}
