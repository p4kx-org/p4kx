// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_gltf::MorphTarget;
use p4kx_model::{Mesh, Model};

use super::ArmourPart;
use super::face_build::Face;
use super::follow::follow;
use super::follow_joints::HeadSkin;
use super::follow_morphs::{Followed, follow_morphs};
use crate::{Issue, IssueKind};

/// The head as it was made and as it was shaped, with its morph targets, which what follows the
/// head moves with.
pub(super) struct HeadMove {
    pub(super) before: Arc<Model>,
    pub(super) after: Arc<Model>,
    pub(super) morphs: Vec<MorphTarget>,
}

/// A job as it is worn: its model, its morph targets, the face of a head, and, for what follows
/// the head, the target of the head each of its morph targets stands for.
pub(super) struct Worn {
    pub(super) model: Arc<Model>,
    pub(super) morphs: Vec<MorphTarget>,
    pub(super) face: Option<Face>,
    pub(super) follows: Vec<usize>,
}

/// Leaves out of the animation the parts that follow the head whose morph targets do not match the
/// blendshapes of the head's face link, and lists each in `issues`.
pub(super) fn check_follows(parts: &mut [ArmourPart], issues: &mut Vec<Issue>) {
    let Some(known) = parts
        .iter()
        .find_map(|part| part.face.as_deref())
        .map(|link| link.blendshapes.len())
    else {
        return;
    };
    for part in parts.iter_mut().filter(|part| !part.follows.is_empty()) {
        let reason = if part.follows.len() != part.morphs.len() {
            "the targets of the part and of the head do not correspond"
        } else if part.follows.iter().any(|&target| target >= known) {
            "a target stands for a blendshape the head lacks"
        } else {
            continue;
        };
        part.follows.clear();
        issues.push(Issue {
            path: String::new(),
            name: part.name.clone(),
            kind: IssueKind::FollowWeightsSkipped {
                reason: reason.to_owned(),
            },
        });
    }
}

/// Returns the meshes of `model` at level 0.
pub(super) fn level_zero_meshes(model: &Model) -> usize {
    model
        .lods
        .first()
        .filter(|lod| lod.level == 0)
        .map_or(0, |lod| lod.meshes.len())
}

fn first_mesh(model: &Model) -> Option<&Mesh> {
    model.lods.first().and_then(|lod| lod.meshes.first())
}

/// Returns `model` with its vertices following the head as it was shaped, and with morph targets
/// that follow the expression targets of the head.
pub(super) fn followed(model: &Arc<Model>, head: &HeadMove) -> Worn {
    let (model, moved) = match (first_mesh(&head.before), first_mesh(&head.after)) {
        (Some(before), Some(after)) => {
            let skin = HeadSkin::of(&head.before, before);
            (
                Arc::new(follow(model, before, after, skin.as_ref())),
                follow_morphs(model, before, &head.morphs),
            )
        }
        _ => (Arc::clone(model), Followed::none()),
    };
    Worn {
        model,
        morphs: moved.targets,
        face: None,
        follows: moved.of,
    }
}
