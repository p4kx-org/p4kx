// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gltf::RigMount;
use p4kx_model::{Bone, BoneMatch, Model};

use super::ArmourPart;
use super::jobs::{Job, issue};
use super::layered_worn::OverlayMount;
use super::mount_source::{PlacementModel, WornMountSource};
use super::piece::Mount;
use super::read_models::Loaded;
use crate::placement::{bone_placement, is_unskinned, node_placement, offset_transform};
use crate::{Issue, IssueKind, PortPlacementSource};

/// The turn about X in degrees from the frame of a bone, which has Z up, to the frame of the
/// helper of an item, which has Y up (ADR-0231).
const BONE_TO_HELPER: [f32; 3] = [90.0, 0.0, 0.0];

#[derive(Debug, Clone)]
pub(super) struct Placed {
    pub(super) frame: RigMount,
    pub(super) source: WornMountSource,
}

/// Hangs the unskinned parts that have a mount on the bone of the skeleton their port is
/// attached to; a helper that is no bone of its holder is listed and leaves the part at the origin.
pub(super) fn place(
    jobs: &[&Job],
    parts: &mut [ArmourPart],
    skeleton: PlacementModel<'_>,
    level: u32,
    issues: &mut Vec<Issue>,
) {
    for (index, job) in jobs.iter().enumerate() {
        if !parts
            .get(index)
            .is_some_and(|part| is_unskinned(&part.model, level))
        {
            continue;
        }
        let Some(mount) = &job.mount else { continue };
        match mounted(parts, jobs, index, mount, skeleton, level) {
            Ok(placed) => {
                if let Some(diagnostic) = &mount.helper_diagnostic {
                    issues.push(issue(
                        job,
                        IssueKind::PortHelperUnresolved(diagnostic.clone()),
                    ));
                }
                if let Some(part) = parts.get_mut(index) {
                    part.mount = Some(Box::new(placed));
                }
            }
            Err(_) if job.overlay_mount.requires_mount() => issues.push(issue(
                job,
                IssueKind::WornMountUnresolved(
                    "the current holder or its authored helper is absent from the exported model",
                ),
            )),
            Err(helper) => issues.push(issue(job, IssueKind::HelperNotFound { helper })),
        }
    }
}

pub(super) fn omit_unbound(
    jobs: &[Job],
    models: &mut [Option<&Loaded>],
    level: u32,
    issues: &mut Vec<Issue>,
) {
    for (job, model) in jobs.iter().zip(models) {
        if let OverlayMount::Unresolved(reason) = job.overlay_mount
            && model.is_some_and(|model| is_unskinned(&model.model, level))
        {
            issues.push(issue(job, IssueKind::WornMountUnresolved(reason.message())));
            *model = None;
        }
    }
}

pub(super) fn included(part: ArmourPart, job: &Job, level: u32) -> Option<ArmourPart> {
    (!job.overlay_mount.requires_mount()
        || part.mount.is_some()
        || !is_unskinned(&part.model, level))
    .then_some(part)
}

/// Returns the bones of the first level of `model`, the ones [`bone_placement`] looks in.
fn bones_of(model: &Model) -> &[Bone] {
    model
        .lods
        .first()
        .map_or(model.bones.as_slice(), |lod| model.bones_of(lod))
}

/// Returns the model that holds the helper of `mount`: the skeleton of the body, or the piece it
/// names; `None` when that piece is not worn.
fn holder<'a>(
    parts: &'a [ArmourPart],
    jobs: &[&Job],
    mount: &Mount,
    binding: OverlayMount,
    skeleton: PlacementModel<'a>,
    level: u32,
) -> Option<PlacementModel<'a>> {
    let Some(name) = &mount.holder else {
        return Some(skeleton);
    };
    if binding == OverlayMount::Bound {
        let index = mount.holder_index?;
        let mut matches = parts
            .iter()
            .zip(jobs)
            .filter(|(part, job)| job.overlay_index == Some(index) && part.name == *name);
        let (part, job) = matches.next()?;
        if matches.next().is_some()
            || (job.overlay_mount.requires_mount() && is_unskinned(&part.model, level))
        {
            return None;
        }
        return Some(PlacementModel::part(part));
    }
    parts
        .iter()
        .find(|part| part.name == *name)
        .map(PlacementModel::part)
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
    jobs: &[&Job],
    index: usize,
    mount: &Mount,
    skeleton: PlacementModel<'_>,
    level: u32,
) -> Result<Placed, String> {
    let missing = || mount.helper.clone();
    let binding = jobs.get(index).ok_or_else(missing)?.overlay_mount;
    let holder = holder(parts, jobs, mount, binding, skeleton, level).ok_or_else(missing)?;
    let held = bones_of(holder.model);
    let at = held
        .iter()
        .position(|bone| bone.name.eq_ignore_ascii_case(&mount.helper))
        .ok_or_else(missing)?;
    let bone = usize::from(
        BoneMatch::new(&skeleton.model.bones, held)
            .target(u16::try_from(at).map_err(|_| missing())?),
    );
    let anchor = skeleton
        .model
        .bones
        .get(bone)
        .and_then(|anchor| bone_placement(skeleton.model, &anchor.name))
        .ok_or_else(missing)?;
    let inverse = anchor.transform.inverse().ok_or_else(missing)?;
    let held = bone_placement(holder.model, &mount.helper).ok_or_else(missing)?;
    let own = parts.get(index).and_then(|part| {
        if mount.offset_helper.is_empty() {
            return None;
        }
        let placement = node_placement(&part.model, &mount.offset_helper)?;
        Some((
            placement.transform.inverse()?,
            PlacementModel::part(part).node_source(placement.node),
        ))
    });
    let at_port = inverse
        .then(&held.transform)
        .then(&offset_transform([0.0; 3], BONE_TO_HELPER))
        .then(&offset_transform(mount.position(), mount.rotation()));
    let port = PortPlacementSource::worn(mount.port, mount.offset_fields, own.is_some());
    let (transform, locator) = own.map_or((at_port, None), |(inner, source)| {
        (at_port.then(&inner), source)
    });
    Ok(Placed {
        frame: RigMount::new(bone, transform),
        source: WornMountSource::new(
            port,
            holder.bone_source(held.selection),
            skeleton.bone_source(anchor.selection),
            locator,
        ),
    })
}
