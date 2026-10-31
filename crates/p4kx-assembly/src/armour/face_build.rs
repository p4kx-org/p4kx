// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_character::Dna;
use p4kx_model::{BoneMatch, Model, extend_skeleton};

use super::ArmourPart;
use super::facial::{FaceLink, FaceSource, Facial, bones_of};
use super::jobs::{Job, issue};
use super::mount::place;
use crate::placement::is_unskinned;
use crate::{Issue, IssueKind};

/// What a shaped head brings to the skeleton: where its rig's joints are among its bones, and the
/// blendshape number of each of its morph targets.
pub(super) struct Face {
    pub(super) facial: Option<Facial>,
    pub(super) rig: String,
    pub(super) numbers: Vec<usize>,
}

impl Face {
    /// Returns the face of a head from the joints and rig path it was made with, if it was, and
    /// the blendshape number of each of its morph targets.
    pub(super) fn made(face: Option<(Facial, String)>, numbers: Vec<usize>) -> Self {
        let (facial, rig) = face.unzip();
        Self {
            facial,
            rig: rig.unwrap_or_default(),
            numbers,
        }
    }
}

/// A head part and what ties it to its rig.
pub(super) struct Found {
    pub(super) part: usize,
    pub(super) source: Option<FaceSource>,
    pub(super) face: Face,
}

impl Found {
    /// Returns the part `part` made of the skin of a story character's own head: its bones join
    /// the skeleton at the positions its skin gives. It is animated when it has `face`, the joints
    /// of the character's own `.RigLogic` file.
    pub(super) fn bespoke(part: usize, face: Option<Face>) -> Self {
        let source = face
            .as_ref()
            .filter(|face| face.facial.is_some())
            .map(|_| FaceSource::Own);
        Self {
            part,
            source,
            face: face.unwrap_or(Face {
                facial: None,
                rig: String::new(),
                numbers: Vec::new(),
            }),
        }
    }

    /// Returns the head part `part` when it was shaped by `dna` and has a face.
    pub(super) fn of(part: usize, dna: Option<&Dna>, face: Option<Face>) -> Option<Self> {
        let face = face?;
        Some(Self {
            part,
            source: Some(FaceSource::Dna {
                rig: face.rig.clone(),
                dna: dna?.clone(),
            }),
            face,
        })
    }
}

/// Appends the bones of each head to the skeleton, moved to the Character's own joint positions
/// where the rig gives them, and ties each head part to its rig.
pub(super) fn extend_with_faces(skeleton: &mut Model, parts: &mut [ArmourPart], found: Vec<Found>) {
    for mut head in found {
        let Some(part) = parts.get_mut(head.part) else {
            continue;
        };
        let posed = head
            .face
            .facial
            .as_mut()
            .and_then(|facial| facial.posed.take());
        let bones = posed.unwrap_or_else(|| bones_of(&part.model).to_vec());
        let moves = head
            .face
            .facial
            .as_ref()
            .map(|facial| facial.offsets.as_slice())
            .unwrap_or_default();
        skeleton.bones = extend_skeleton(&skeleton.bones, &bones, moves);
        let (Some(facial), Some(source)) = (head.face.facial, head.source) else {
            continue;
        };
        let mut by_name: HashMap<String, usize> = HashMap::with_capacity(skeleton.bones.len());
        for (index, bone) in skeleton.bones.iter().enumerate() {
            by_name
                .entry(bone.name.to_ascii_lowercase())
                .or_insert(index);
        }
        let joints = facial
            .joints
            .iter()
            .map(|&joint| {
                bones
                    .get(joint)
                    .and_then(|bone| by_name.get(&bone.name.to_ascii_lowercase()))
                    .copied()
                    .unwrap_or(usize::MAX)
            })
            .collect();
        part.face = Some(
            FaceLink {
                source,
                joints,
                blendshapes: head.face.numbers,
                logic: facial.logic,
            }
            .shared(),
        );
    }
}

/// Returns `skeleton` with the bones of the heads appended and notes how each part fits it.
pub(super) fn settled(
    mut skeleton: Model,
    parts: &mut [ArmourPart],
    heads: Vec<Found>,
    jobs: &[&Job],
    level: u32,
    issues: &mut Vec<Issue>,
) -> Model {
    extend_with_faces(&mut skeleton, parts, heads);
    place(jobs, parts, &skeleton, level, issues);
    for (job, part) in jobs.iter().zip(parts.iter()) {
        note_fit(job, part, &skeleton, level, issues);
    }
    skeleton
}

/// Notes how a part fits the skeleton at the level `level` of detail that is exported: bones it
/// lacks, and a model that is not skinned at all.
fn note_fit(job: &Job, part: &ArmourPart, skeleton: &Model, level: u32, issues: &mut Vec<Issue>) {
    let model = &part.model;
    let Some(lod) = model.nearest_lod(level) else {
        return;
    };
    if is_unskinned(model, level) {
        if part.mount.is_none() {
            issues.push(issue(job, IssueKind::NotSkinned));
        }
        return;
    }
    let matched = BoneMatch::new(&skeleton.bones, model.bones_of(lod));
    let count = matched.unmatched().len();
    if count > 0 {
        issues.push(issue(job, IssueKind::BonesNotInSkeleton { count }));
    }
}
