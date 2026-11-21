// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use crate::{Bone, BoneTransform, Transform};

/// Binding of an authored attachment name to a physical skeleton.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum AttachmentBoneBinding {
    /// The selector matches no bone.
    Missing,
    /// Several original bones match the selector.
    Ambiguous {
        /// Matching physical ordinals in source order.
        ordinals: Vec<usize>,
    },
    /// A unique bone exists but its model-space frame could not be proved.
    UnavailableFrame {
        /// Original ordinal of the selected bone.
        ordinal: usize,
        /// Consulted physical bones in order from the selected bone towards the root.
        lineage: Vec<usize>,
    },
    /// A unique bone with a complete, finite bind-pose route.
    Bound {
        /// Original ordinal of the selected bone.
        ordinal: usize,
        /// Physical bones in order from the selected bone to the root, inclusive.
        lineage: Vec<usize>,
        /// The bone-to-model affine in Z-up metres.
        transform: Transform,
    },
}

/// Resolves a nonempty attachment bone name with ASCII case-insensitive matching.
#[must_use]
pub fn resolve_attachment_bone(bones: &[Bone], name: &str) -> AttachmentBoneBinding {
    if name.is_empty() {
        return AttachmentBoneBinding::Missing;
    }
    let mut matching = bones
        .iter()
        .enumerate()
        .filter_map(|(ordinal, bone)| bone.name.eq_ignore_ascii_case(name).then_some(ordinal));
    let Some(ordinal) = matching.next() else {
        return AttachmentBoneBinding::Missing;
    };
    if let Some(second) = matching.next() {
        let mut ordinals = vec![ordinal, second];
        ordinals.extend(matching);
        return AttachmentBoneBinding::Ambiguous { ordinals };
    }

    let mut lineage = Vec::new();
    match bind_pose(bones, ordinal, &mut lineage) {
        Some(transform) => AttachmentBoneBinding::Bound {
            ordinal,
            lineage,
            transform,
        },
        None => AttachmentBoneBinding::UnavailableFrame { ordinal, lineage },
    }
}

/// Converts the authored bone-relative translation in metres and WXYZ quaternion to an affine.
#[must_use]
pub fn attachment_offset(position: [f32; 3], rotation_wxyz: [f32; 4]) -> Option<Transform> {
    Transform::from_quaternion(rotation_wxyz.map(f64::from), position.map(f64::from))
}

fn bind_pose(bones: &[Bone], ordinal: usize, lineage: &mut Vec<usize>) -> Option<Transform> {
    let mut active = HashSet::new();
    let mut next = Some(ordinal);
    while let Some(at) = next {
        let bone = bones.get(at)?;
        if !active.insert(at) {
            return None;
        }
        lineage.push(at);
        next = bone.parent;
    }

    let pose = lineage
        .iter()
        .rev()
        .try_fold(BoneTransform::IDENTITY, |parent, &member| {
            bones.get(member).map(|bone| parent.then(&bone.relative))
        })?;
    let [x, y, z, w] = pose.rotation;
    attachment_offset(pose.translation, [w, x, y, z])
}

#[cfg(test)]
#[path = "attachment_bone_binding_tests.rs"]
mod tests;
