// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The skeleton of a character Entity: the bones of the root Entity's own model. The skins of a
//! creature (a character file outside the human rigs) are bound to it; the item ports of the skins
//! of any character root, a human rig included, anchor to it.

use std::sync::Arc;

use p4kx_model::{Bone, BoneMatch, Model, Transform};

use crate::character_path::{is_character_path, is_creature_path};
use crate::placement::bone_pose;
use crate::{Assembly, EntityNode, Issue, IssueKind, LoadoutError, Part, Placement};

/// Returns the bones of the root part's model at the level `level` of detail, or `None` when the
/// root's geometry is no creature file (a character file below `Objects/Characters` outside the
/// human rigs), its skeleton file is missing or unreadable, or its model has no bones.
///
/// The bones are in the root's space, which is the Assembly's: the root sits at the identity and
/// a skin part is placed at the identity too (ADR-0224), so the skins agree with the bones. A
/// skinned mesh ignores the transform of its node in glTF, so the port offset of a skin part has
/// no effect.
pub(crate) fn skeleton_of<'a>(
    parts: &'a [Part],
    issues: &[Issue],
    level: u32,
) -> Option<&'a [Bone]> {
    bones_of_root(parts, issues, level, is_creature_path)
}

impl Assembly {
    /// Returns the parts of a creature whose skin is bound to its skeleton, the root included
    /// when it draws a skin, or `None` when the Assembly writes no creature skeleton: the root's
    /// geometry is no creature file, its skeleton file is missing or unreadable, or it has no
    /// bones. [`Assembly::write_glb`] writes the skeleton as bone nodes then.
    #[must_use]
    pub fn creature_skins(&self) -> Option<impl Iterator<Item = &Part>> {
        skeleton_of(&self.parts, &self.issues, self.lod)?;
        let level = self.lod;
        Some(self.parts.iter().filter(move |part| {
            part.model
                .as_deref()
                .is_some_and(|model| is_skinned(model, level))
        }))
    }
}

/// Returns the bones the item ports of the skins anchor to: those of the root part's model when
/// its geometry is any character file, a human body rig included, at the level `level` of detail.
/// The skins of a human root are not bound to these bones, but the props on its ports hang on them.
pub(crate) fn anchor_bones_of<'a>(
    parts: &'a [Part],
    issues: &[Issue],
    level: u32,
) -> Option<&'a [Bone]> {
    bones_of_root(parts, issues, level, is_character_path)
}

/// Returns the bones of the root part's model at the level `level` when [`rig_model`] finds one
/// for `accepts`, or `None` when the level is missing or the model has no bones.
fn bones_of_root<'a>(
    parts: &'a [Part],
    issues: &[Issue],
    level: u32,
    accepts: fn(&str) -> bool,
) -> Option<&'a [Bone]> {
    let model = rig_model(parts, issues, accepts)?;
    let bones = model.bones_of(model.nearest_lod(level)?);
    (!bones.is_empty()).then_some(bones)
}

/// Returns the model of the root part when its geometry is a file `accepts` and its skeleton file
/// is present: the model whose bones the skins of a creature are bound to and the item ports of
/// the skins anchor to.
fn rig_model<'a>(
    parts: &'a [Part],
    issues: &[Issue],
    accepts: fn(&str) -> bool,
) -> Option<&'a Arc<Model>> {
    let root = parts.first()?;
    if !root.geometry.as_deref().is_some_and(accepts)
        || issues.iter().any(|issue| {
            issue.path.is_empty()
                && matches!(
                    issue.kind,
                    IssueKind::SkeletonMissing { .. } | IssueKind::SkeletonUnreadable { .. }
                )
        })
    {
        return None;
    }
    root.model.as_ref()
}

/// Anchors the item port of a skin part to the bone `helper` of the character skeleton `bones`
/// (from [`anchor_bones_of`]): the bind pose of the bone in the space of `parent`, a skin part.
/// Returns `None` when `parent` is no skin part or the skeleton lacks the bone.
pub(crate) fn skin_port_anchor(
    bones: &[Bone],
    parent: &Part,
    helper: &str,
) -> Option<(Placement, Transform)> {
    if parent.placement != Placement::Skin {
        return None;
    }
    let pose = bone_pose(bones, helper)?;
    let transform = parent.world.inverse()?.then(&pose);
    Some((Placement::Helper(helper.to_owned()), transform))
}

/// Lists, per skinned part below the root, the bones of its model that the root's skeleton lacks;
/// their vertices follow the nearest parent bone the skeleton has.
pub(crate) fn note_fit(parts: &[Part], level: u32, issues: &mut Vec<Issue>) {
    let Some(skeleton) = skeleton_of(parts, issues, level) else {
        return;
    };
    let notes: Vec<Issue> = parts
        .iter()
        .skip(1)
        .filter_map(|part| {
            let model = part
                .model
                .as_deref()
                .filter(|model| is_skinned(model, level))?;
            let lod = model.nearest_lod(level)?;
            let count = BoneMatch::new(skeleton, model.bones_of(lod))
                .unmatched()
                .len();
            (count > 0).then(|| Issue {
                path: part.path.clone(),
                name: part.class_name.clone(),
                kind: IssueKind::BonesNotInSkeleton { count },
            })
        })
        .collect();
    issues.extend(notes);
}

/// Lists what the Loadout of a creature root could not read, such as an XML Loadout the Archive
/// lacks, so that a creature that draws nothing says why. Other Entities list nothing here.
pub(crate) fn note_loadout(root: &EntityNode, issues: &mut Vec<Issue>) {
    if !root
        .geometry()
        .is_some_and(|geometry| is_creature_path(geometry.path()))
    {
        return;
    }
    let notes = root
        .issues()
        .iter()
        .filter(|issue| {
            !matches!(
                issue,
                LoadoutError::AmbiguousClass { .. } | LoadoutError::VehicleImplementation { .. }
            )
        })
        .map(|issue| Issue {
            path: String::new(),
            name: root.class_name().to_owned(),
            kind: IssueKind::LoadoutUnreadable {
                reason: issue.to_string(),
            },
        });
    issues.extend(notes);
}

pub(crate) fn is_skinned(model: &Model, level: u32) -> bool {
    model
        .nearest_lod(level)
        .is_some_and(|lod| lod.meshes.iter().any(|mesh| mesh.influences.is_some()))
}
