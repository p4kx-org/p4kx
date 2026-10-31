// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gltf::RigMount;
use p4kx_model::{Bone, BoneMatch, Model};

use super::ArmourPart;
use super::jobs::{Job, issue};
use super::piece::Mount;
use crate::placement::{bone_transform, is_unskinned, node_transform, offset};
use crate::{Issue, IssueKind};

/// The turn about X in degrees from the frame of a bone, which has Z up, to the frame of the
/// helper of an item, which has Y up (ADR-0231).
const BONE_TO_HELPER: [f32; 3] = [90.0, 0.0, 0.0];

/// Hangs the unskinned parts that have a mount on the bone of the skeleton their port is
/// attached to; a helper that is no bone of its holder is listed and leaves the part at the origin.
pub(super) fn place(
    jobs: &[&Job],
    parts: &mut [ArmourPart],
    skeleton: &Model,
    level: u32,
    issues: &mut Vec<Issue>,
) {
    for (index, job) in jobs.iter().enumerate() {
        let Some(mount) = &job.mount else { continue };
        if !parts
            .get(index)
            .is_some_and(|part| is_unskinned(&part.model, level))
        {
            continue;
        }
        match mounted(parts, index, mount, skeleton) {
            Ok(placed) => {
                if let Some(part) = parts.get_mut(index) {
                    part.mount = Some(placed);
                }
            }
            Err(helper) => issues.push(issue(job, IssueKind::HelperNotFound { helper })),
        }
    }
}

/// Returns the bones of the first level of `model`, the ones [`bone_transform`] looks in.
fn bones_of(model: &Model) -> &[Bone] {
    model
        .lods
        .first()
        .map_or(model.bones.as_slice(), |lod| model.bones_of(lod))
}

/// Returns the model that holds the helper of `mount`: the skeleton of the body, or the piece it
/// names; `None` when that piece is not worn.
fn holder<'a>(parts: &'a [ArmourPart], mount: &Mount, skeleton: &'a Model) -> Option<&'a Model> {
    let Some(name) = &mount.holder else {
        return Some(skeleton);
    };
    parts
        .iter()
        .find(|part| part.name == *name)
        .map(|part| &*part.model)
}

/// Returns the bone of the skeleton the part `index` hangs on and where it sits below it: the
/// bone of the holder that the port names, or the skeleton's nearest bone above it, and the
/// offset of the port and of the part's own helper.
///
/// # Errors
///
/// Returns the name of the helper when the holder has no such bone or it cannot be placed.
fn mounted(
    parts: &[ArmourPart],
    index: usize,
    mount: &Mount,
    skeleton: &Model,
) -> Result<RigMount, String> {
    let missing = || mount.helper.clone();
    let holder = holder(parts, mount, skeleton).ok_or_else(missing)?;
    let held = bones_of(holder);
    let at = held
        .iter()
        .position(|bone| bone.name.eq_ignore_ascii_case(&mount.helper))
        .ok_or_else(missing)?;
    let bone = usize::from(
        BoneMatch::new(&skeleton.bones, held).target(u16::try_from(at).map_err(|_| missing())?),
    );
    let anchor = skeleton
        .bones
        .get(bone)
        .and_then(|anchor| bone_transform(skeleton, &anchor.name))
        .and_then(|pose| pose.inverse());
    let (Some(held_pose), Some(anchor)) = (bone_transform(holder, &mount.helper), anchor) else {
        return Err(missing());
    };
    let own = parts.get(index).and_then(|part| {
        let name = mount.offset_helper.as_str();
        (!name.is_empty())
            .then(|| node_transform(&part.model, name)?.inverse())
            .flatten()
    });
    let at_port = anchor
        .then(&held_pose)
        .then(&offset([0.0; 3], BONE_TO_HELPER))
        .then(&offset(mount.position(), mount.rotation()));
    Ok(RigMount::new(
        bone,
        own.map_or(at_port, |inner| at_port.then(&inner)),
    ))
}
