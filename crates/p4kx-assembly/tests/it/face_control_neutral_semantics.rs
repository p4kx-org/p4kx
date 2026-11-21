// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::ArchiveModels;
use p4kx_gltf::SampledAnimation;
use p4kx_model::Animation;
use p4kx_testkit::{TestAnimation, TestRotationFormat, TestTrack};
use serde_json::{Value, json};

use super::{TestResult, animation, axis, fixture, track};

fn assert_standalone_pose(first: &SampledAnimation, second: &SampledAnimation) {
    assert_eq!(
        first.times, second.times,
        "same actual scalar sampling frames"
    );
    assert_eq!(
        first.bones, second.bones,
        "unkeyed control keeps the modeled neutral joint pose"
    );
    assert_eq!(
        first.weights, second.weights,
        "unkeyed control keeps the modeled neutral morph activations"
    );
}

fn verify_unkeyed_placeholder(clip: &Animation, controller: u32) -> TestResult {
    let extra = clip
        .tracks
        .iter()
        .find(|track| track.controller_id == controller)
        .ok_or("the known placeholder control")?;
    assert!(
        extra.translations.is_none(),
        "the source descriptor carries no scalar keys"
    );
    assert_eq!(
        extra.rotations.as_ref().map(|keys| keys.frames.len()),
        Some(4),
        "four retained identity keys"
    );
    Ok(())
}

#[test]
fn unkeyed_control_and_identity_only_placeholder_have_the_same_standalone_pose() -> TestResult {
    let npc = fixture()?;
    let models = ArchiveModels::new(&npc.archive);
    let active = animation(&TestAnimation::new("face.caf", 30).track(track("CTL_a")))?;
    let identity = [0.0, 0.0, 0.0, 1.0];
    let placeholder = TestTrack::new(crc32fast::hash(b"CTL_b")).rotations(&[
        (0, identity),
        (6, identity),
        (13, identity),
        (20, identity),
    ]);
    let with_placeholder = animation(
        &TestAnimation::new("face.caf", 30)
            .track(track("CTL_a"))
            .track(placeholder),
    )?;
    verify_unkeyed_placeholder(&with_placeholder, crc32fast::hash(b"CTL_b"))?;
    let first = npc.armour.animate(std::slice::from_ref(&active), &models);
    let second = npc
        .armour
        .animate(std::slice::from_ref(&with_placeholder), &models);
    let [first] = first.sampled() else {
        return Err("one conventional facial animation for the absent control".into());
    };
    let [second] = second.sampled() else {
        return Err("one conventional facial animation for the placeholder".into());
    };
    assert_standalone_pose(first, second);
    assert!(
        first.skipped_tracks.is_empty(),
        "an entirely absent rig control is already accepted by the modeled standalone evaluator"
    );
    assert_eq!(
        second.skipped_tracks,
        [crc32fast::hash(b"CTL_b")],
        "the only bookkeeping difference is the known scalar-less placeholder"
    );
    npc.armour
        .verify_face_rotations(&active, &models, &[0, 10, 20])?;
    let failure = npc
        .armour
        .verify_face_rotations(&with_placeholder, &models, &[0, 10, 20])
        .err()
        .ok_or("the unchanged strict entry point must still refuse the placeholder")?;
    let (_, detail) = failure
        .split_once("; controllerBinding=")
        .ok_or("exact failure detail")?;
    let detail: Value = serde_json::from_str(detail)?;
    assert_eq!(
        detail.pointer("/groups/missingTranslation"),
        Some(&json!(1)),
        "missing scalar is explicit rather than inferred runtime state"
    );
    Ok(())
}

#[test]
fn varying_rotation_without_scalar_stays_outside_strict_verification() -> TestResult {
    let npc = fixture()?;
    let models = ArchiveModels::new(&npc.archive);
    let active = TestTrack::new(crc32fast::hash(b"CTL_b"))
        .rotation_format(TestRotationFormat::Float)
        .rotations(&[
            (0, [0.0, 0.0, 0.0, 1.0]),
            (10, axis(0, 0.5)),
            (20, [0.0, 0.0, 0.0, 1.0]),
        ]);
    let clip = animation(
        &TestAnimation::new("face.caf", 30)
            .track(track("CTL_a"))
            .track(active),
    )?;
    let original = clip
        .tracks
        .iter()
        .find(|track| track.controller_id == crc32fast::hash(b"CTL_b"))
        .ok_or("known rotating control")?;
    assert!(original.translations.is_none(), "no scalar payload");
    let rotations = original.rotations.as_ref().ok_or("real rotation payload")?;
    assert!(
        rotations.values.windows(2).any(|pair| matches!(
            pair,
            [first, second] if first.map(f32::to_bits) != second.map(f32::to_bits)
        )),
        "the source carries varying rotations and cannot be classified as a neutral placeholder"
    );
    let failure = npc
        .armour
        .verify_face_rotations(&clip, &models, &[0, 10, 20])
        .err()
        .ok_or("an active rotation cannot become a neutral scalar by omission")?;
    assert!(
        failure.starts_with(
            "comparison needs a completely bound facial control sequence; controllerBinding="
        ),
        "the strict path still refuses unsupported scalar-less animation data"
    );
    Ok(())
}
