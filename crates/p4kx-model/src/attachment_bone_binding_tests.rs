// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::f32::consts::FRAC_1_SQRT_2;

use crate::{Bone, BoneTransform};

use super::{AttachmentBoneBinding, attachment_offset, resolve_attachment_bone};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn bound_attachment_uses_every_original_ancestor_and_its_relative_pose() -> TestResult {
    let mut root = bone("root", None, [4.0, 5.0, 6.0]);
    root.relative.rotation = [0.0, 0.0, FRAC_1_SQRT_2, FRAC_1_SQRT_2];
    let bones = [
        bone("Mount", Some(2), [0.0, 3.0, 0.0]),
        root,
        bone("joint", Some(1), [2.0, 0.0, 0.0]),
    ];
    let AttachmentBoneBinding::Bound {
        ordinal,
        lineage,
        transform,
    } = resolve_attachment_bone(&bones, "mount")
    else {
        return Err("one bone with a complete hierarchy must bind".into());
    };
    assert_eq!(ordinal, 0, "physical matching occurrence");
    assert_eq!(lineage, [0, 2, 1], "selected bone to physical root order");
    let offset =
        attachment_offset([1.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0]).ok_or("finite CDF offset")?;
    let attachment = transform.then(&offset);
    for (actual, expected) in attachment.translation().into_iter().zip([1.0, 8.0, 6.0]) {
        assert!(
            (actual - expected).abs() < 1e-5,
            "{actual} versus {expected}"
        );
    }
    Ok(())
}

#[test]
fn duplicate_bones_are_reported_in_original_order() {
    let bones = [
        bone("mount", None, [0.0; 3]),
        bone("other", None, [0.0; 3]),
        bone("MOUNT", None, [0.0; 3]),
        bone("Mount", None, [0.0; 3]),
    ];
    assert_eq!(
        resolve_attachment_bone(&bones, "mount"),
        AttachmentBoneBinding::Ambiguous {
            ordinals: vec![0, 2, 3]
        },
        "all physical matches survive"
    );
    assert_eq!(
        resolve_attachment_bone(&bones, "absent"),
        AttachmentBoneBinding::Missing,
        "a valid selector without a match is distinct"
    );
}

#[test]
fn invalid_parent_routes_keep_consulted_ordinals_without_a_frame() {
    for (parent, related, lineage) in [(Some(2), None, vec![0]), (Some(1), Some(0), vec![0, 1])] {
        let bones = [
            bone("Mount", parent, [0.0; 3]),
            bone("joint", related, [0.0; 3]),
        ];
        assert_eq!(
            resolve_attachment_bone(&bones, "mount"),
            AttachmentBoneBinding::UnavailableFrame {
                ordinal: 0,
                lineage
            },
            "missing or cyclic ancestors cannot supply a placement"
        );
    }
}

#[test]
fn invalid_pose_retains_the_complete_physical_route() {
    let mut root = bone("mount", None, [0.0; 3]);
    root.relative.rotation = [0.0; 4];
    assert_eq!(
        resolve_attachment_bone(&[root], "mount"),
        AttachmentBoneBinding::UnavailableFrame {
            ordinal: 0,
            lineage: vec![0]
        },
        "a zero quaternion supplies no transform"
    );
}

fn bone(name: &str, parent: Option<usize>, translation: [f32; 3]) -> Bone {
    Bone {
        name: name.to_owned(),
        parent,
        controller_id: 0,
        node: None,
        relative: BoneTransform {
            translation,
            ..BoneTransform::IDENTITY
        },
        world: BoneTransform::IDENTITY,
    }
}
