// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use crate::Transform;

use super::{AttachmentPose, AttachmentPoseField};

type TestResult = Result<(), Box<dyn Error>>;

const BIND: Transform = Transform {
    rows: [
        [0.0, -1.0, 0.0, 10.0],
        [1.0, 0.0, 0.0, 20.0],
        [0.0, 0.0, 1.0, 30.0],
    ],
};

fn check_frame(pose: AttachmentPose, expected: [[f32; 4]; 3]) -> TestResult {
    let placement = pose
        .selection()
        .in_model_space(
            pose.position.ok_or("the selected position")?,
            pose.rotation_wxyz.ok_or("the selected orientation")?,
            &BIND,
        )
        .ok_or("the attachment model-space frame")?;
    for (actual, wanted) in placement
        .rows
        .into_iter()
        .flatten()
        .zip(expected.into_iter().flatten())
    {
        assert!(
            (actual - wanted).abs() < 1e-5,
            "{:?} against {expected:?}",
            placement.rows
        );
    }
    Ok(())
}

#[test]
fn model_translation_and_relative_rotation_have_independent_frames() -> TestResult {
    let pose = AttachmentPose::read(|field| match field {
        AttachmentPoseField::ModelPosition => Some("100,200,300"),
        AttachmentPoseField::RelativeRotation => Some("1,0,1,0"),
        AttachmentPoseField::ModelRotation => Some("malformed shadowed quaternion"),
        AttachmentPoseField::RelativePosition => None,
    });
    let selection = pose.selection();
    assert!(
        selection.uses(AttachmentPoseField::ModelPosition)
            && selection.uses(AttachmentPoseField::RelativeRotation)
            && !selection.uses(AttachmentPoseField::ModelRotation),
        "only the contributing attributes supply the selected pose"
    );
    check_frame(
        pose,
        [
            [0.0, -1.0, 0.0, 100.0],
            [0.0, 0.0, 1.0, 200.0],
            [-1.0, 0.0, 0.0, 300.0],
        ],
    )
}

#[test]
fn model_rotation_does_not_replace_a_relative_translation() -> TestResult {
    let pose = AttachmentPose::read(|field| match field {
        AttachmentPoseField::RelativePosition => Some("2,3,5"),
        AttachmentPoseField::ModelPosition => Some("100,200,300"),
        AttachmentPoseField::ModelRotation => Some("1,1,0,0"),
        AttachmentPoseField::RelativeRotation => None,
    });
    assert!(
        !pose.selection().uses(AttachmentPoseField::ModelPosition),
        "the literal model translation was shadowed by the relative attribute"
    );
    check_frame(
        pose,
        [
            [1.0, 0.0, 0.0, 7.0],
            [0.0, 0.0, -1.0, 22.0],
            [0.0, 1.0, 0.0, 35.0],
        ],
    )
}

#[test]
fn absent_relative_pose_keeps_the_bind_frame() -> TestResult {
    check_frame(AttachmentPose::read(|_| None), BIND.rows)
}

#[test]
fn absent_model_rotation_defaults_in_model_space() -> TestResult {
    let pose = AttachmentPose::read(|field| {
        (field == AttachmentPoseField::ModelPosition).then_some("100,200,300")
    });
    assert!(
        !pose.selection().uses_bone(),
        "a model-only pose consumes no bind coefficients"
    );
    check_frame(
        pose,
        [
            [1.0, 0.0, 0.0, 100.0],
            [0.0, 1.0, 0.0, 200.0],
            [0.0, 0.0, 1.0, 300.0],
        ],
    )
}

#[test]
fn an_invalid_winning_relative_attribute_does_not_use_a_model_fallback() {
    let pose = AttachmentPose::read(|field| match field {
        AttachmentPoseField::RelativeRotation => Some("0,0,0,0"),
        AttachmentPoseField::ModelRotation => Some("1,0,0,0"),
        AttachmentPoseField::RelativePosition | AttachmentPoseField::ModelPosition => None,
    });
    assert!(
        pose.rotation_wxyz.is_none(),
        "the selected quaternion is invalid"
    );
    assert!(
        pose.selection().uses(AttachmentPoseField::RelativeRotation),
        "selection retains the invalid original operand"
    );
}
