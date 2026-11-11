// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_gltf::MorphTarget;
use p4kx_model::{Mesh, Model};

use super::face_build::Face;
use super::follow::follow;
use super::follow_blend::HeadSkin;
use super::follow_joints::bind;
use super::follow_morphs::{Followed, follow_morphs};
use super::{ArmourPart, ShapeArchiveSources};
use crate::{Issue, IssueKind};

/// The head as it was made and as it was shaped, with its morph targets, which what follows the
/// head moves with.
pub(super) struct HeadMove {
    pub(super) before: Arc<Model>,
    pub(super) after: Arc<Model>,
    pub(super) morphs: Vec<MorphTarget>,
    pub(super) shape_sources: Option<Arc<ShapeArchiveSources>>,
}

/// A job as it is worn: its model, its morph targets, the face of a head, and, for what follows
/// the head, the target of the head each of its morph targets stands for.
pub(super) struct Worn {
    pub(super) model: Arc<Model>,
    pub(super) morphs: Vec<MorphTarget>,
    pub(super) face: Option<Face>,
    pub(super) follows: Vec<usize>,
    pub(super) shape_sources: Vec<Arc<ShapeArchiveSources>>,
    /// What was not done while the job was worn, for the report.
    pub(super) kinds: Vec<IssueKind>,
}

impl Worn {
    pub(super) fn fitted(
        model: Arc<Model>,
        morphs: Vec<MorphTarget>,
        face: Option<Face>,
        shape_sources: Option<Arc<ShapeArchiveSources>>,
    ) -> Self {
        Self {
            model,
            morphs,
            face,
            shape_sources: shape_sources.into_iter().collect(),
            follows: Vec::new(),
            kinds: Vec::new(),
        }
    }
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

/// Returns the note that only the first mesh of `worn` follows the head's expressions, when `worn`
/// has targets and several meshes at the level of detail it was read at.
pub(super) fn first_mesh_note(worn: &Worn) -> Option<IssueKind> {
    let meshes = worn.model.lods.first().map_or(0, |lod| lod.meshes.len());
    (meshes > 1 && !worn.morphs.is_empty())
        .then_some(IssueKind::FollowMorphsFirstMeshOnly { meshes })
}

/// Returns the first mesh of the level of detail `model` was read at.
pub(super) fn first_mesh(model: &Model) -> Option<&Mesh> {
    model.lods.first().and_then(|lod| lod.meshes.first())
}

/// Returns `model` with its vertices following the head as it was shaped, and with morph targets
/// that follow the expression targets of the head.
///
/// The vertices are bound to the joints of the head as well, see [`bind`]; when that cannot be
/// done, `kinds` of the result says why and the vertices keep their own bones.
pub(super) fn followed(model: &Arc<Model>, head: &HeadMove) -> Worn {
    let mut kinds = Vec::new();
    let (model, moved, shape_sources) = match (first_mesh(&head.before), first_mesh(&head.after)) {
        (Some(before), Some(after)) => {
            let mut bound = Model::clone(model);
            if let Err(reason) =
                HeadSkin::of(&head.before, before).and_then(|skin| bind(&mut bound, &skin))
            {
                kinds.push(IssueKind::FollowJointsSkipped {
                    reason: reason.to_string(),
                });
            }
            (
                Arc::new(follow(bound, before, after)),
                follow_morphs(model, before, &head.morphs),
                head.shape_sources.clone(),
            )
        }
        _ => (Arc::clone(model), Followed::none(), None),
    };
    Worn {
        model,
        morphs: moved.targets,
        face: None,
        follows: moved.of,
        shape_sources: shape_sources.into_iter().collect(),
        kinds,
    }
}
