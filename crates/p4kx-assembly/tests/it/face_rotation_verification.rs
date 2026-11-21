// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "face_control_binding_failures.rs"]
mod binding_failures;
#[path = "face_control_neutral_semantics.rs"]
mod neutral_semantics;

use std::error::Error;

use p4kx_assembly::{ArchiveModels, Expressions, FaceRotationComparison};
use p4kx_gltf::{GlbOptions, SampledAnimation};
use p4kx_model::multiply_quaternions;
use p4kx_testkit::{
    NPC_FACIAL_BONE, NPC_RIG_LOGIC, NPC_SKELETON_SHIFT, TestAnimation, TestEntry, TestLogicRecord,
    TestSkeleton, TestTrack, check_glb, npc_face_entries, npc_face_entries_posed_by,
    npc_face_logic, npc_face_skeleton,
};

use serde_json::json;

use super::{Npc, TestResult, animation, build};

#[test]
fn four_actions_use_the_same_observed_angles_and_export_in_one_glb() -> TestResult {
    let npc = fixture()?;
    let models = ArchiveModels::new(&npc.archive);
    let clips = [animation(
        &TestAnimation::new("face.caf", 30)
            .track(track("CTL_a"))
            .track(track("CTL_b")),
    )?];
    let [clip] = &clips;
    let comparison = npc
        .armour
        .verify_face_rotations(clip, &models, &[0, 10, 20])?;
    let [neutral, peak, last] = comparison.angles_radians() else {
        return Err("the same three sampled frames".into());
    };
    let [[x, y, z]] = peak.as_slice() else {
        return Err("one observed facial joint".into());
    };
    assert_eq!(neutral.as_slice(), [[0.0; 3]], "neutral raw angles");
    assert_eq!(last, neutral, "the clip returns to neutral");
    assert!(
        (x - 0.3).abs() < 1e-5 && (y - 0.5).abs() < 1e-5 && z.abs() < 1e-5,
        "authored radians"
    );
    verify_actions(&npc, comparison.animations().sampled(), [*x, *y])?;
    assert_report_mapping(&npc, &comparison, peak, clip.seconds(10))?;
    let current = npc.armour.animate(&clips, &models);
    let [original] = current.sampled() else {
        return Err("the current convention produces one action".into());
    };
    let first = comparison
        .animations()
        .sampled()
        .first()
        .ok_or("XYZ on the right")?;
    assert_eq!(
        first.bones, original.bones,
        "canonical XYZ/right pose parity"
    );
    assert_eq!(
        first.weights, original.weights,
        "canonical morph-weight parity"
    );

    verify_glb_actions(&npc, &comparison)
}

fn verify_glb_actions(npc: &Npc, comparison: &FaceRotationComparison) -> TestResult {
    let mut bytes = Vec::new();
    npc.armour
        .write_glb_animated(comparison.animations(), &GlbOptions::default(), &mut bytes)?;
    let glb = check_glb(&bytes)?;
    let names: Vec<_> = glb
        .document
        .animations()
        .filter_map(|clip| clip.name())
        .collect();
    assert_eq!(
        names,
        [
            "xyz-bind-times-delta",
            "xyz-delta-times-bind",
            "zyx-bind-times-delta",
            "zyx-delta-times-bind",
        ],
        "four named actions exported with the same skeleton"
    );
    Ok(())
}

fn assert_report_mapping(
    npc: &Npc,
    comparison: &FaceRotationComparison,
    peak: &[[f32; 3]],
    frame_seconds: f32,
) -> TestResult {
    let records: Vec<_> = comparison.verification_records().collect();
    let [header, _, peak_report, _] = records.as_slice() else {
        return Err("one header and three frame records".into());
    };
    let bone = npc
        .armour
        .skeleton()
        .bones
        .iter()
        .enumerate()
        .find(|(_, bone)| bone.name == NPC_FACIAL_BONE)
        .ok_or("fixture's mapped facial bone")?;
    assert_eq!(
        header.pointer("/joints/0"),
        Some(&json!({"rigJoint": 0, "controllerId": bone.1.controller_id,
            "bone": {"index": bone.0, "name": NPC_FACIAL_BONE}})),
        "the report maps original rig ordinals to actual skeleton bones"
    );
    assert_eq!(
        peak_report,
        &json!({"record": "frame", "frame": 10, "seconds": frame_seconds,
            "eulerRadians": peak}),
        "the observed angles are tied to the same exact source frame"
    );
    Ok(())
}

fn verify_actions(npc: &Npc, candidates: &[SampledAnimation], angles: [f32; 2]) -> TestResult {
    let first = candidates.first().ok_or("an action")?;
    let reference = first.bones.first().ok_or("a moving joint")?;
    let bind = npc
        .armour
        .skeleton()
        .bones
        .get(reference.bone)
        .ok_or("the original bind")?
        .relative
        .rotation;
    let expected = compositions(bind, angles);
    let mut prior = Vec::new();
    assert_eq!(
        candidates.len(),
        expected.len(),
        "one action per convention"
    );
    for (candidate, quaternion) in candidates.iter().zip(expected) {
        let [joint] = candidate.bones.as_slice() else {
            return Err("one moving joint per action".into());
        };
        let [rest, peak, last] = joint.rotations.as_slice() else {
            return Err("one rotation per sample".into());
        };
        assert_eq!(
            candidate.times, first.times,
            "same exact source sample times"
        );
        assert_eq!(
            candidate.weights, first.weights,
            "same actual morph weights"
        );
        assert_eq!(
            joint.translations, reference.translations,
            "same actual translation tracks"
        );
        assert!(
            alignment(*rest, bind) > 1.0 - 1e-5,
            "neutral rotation is the bind pose"
        );
        assert!(
            alignment(*last, bind) > 1.0 - 1e-5,
            "last rotation returns to bind"
        );
        assert!(
            alignment(*peak, quaternion) > 1.0 - 1e-5,
            "named composition gives the expected peak"
        );
        assert!(
            prior.iter().all(|&other| alignment(*peak, other) < 0.999_9),
            "this fixture distinguishes all candidates"
        );
        prior.push(*peak);
    }
    Ok(())
}

fn compositions(bind: [f32; 4], [x, y]: [f32; 2]) -> [[f32; 4]; 4] {
    let qx = axis(0, x);
    let qy = axis(1, y);
    let xyz = multiply_quaternions(qx, qy);
    let zyx = multiply_quaternions(qy, qx);
    [
        multiply_quaternions(bind, xyz),
        multiply_quaternions(xyz, bind),
        multiply_quaternions(bind, zyx),
        multiply_quaternions(zyx, bind),
    ]
}

fn fixture() -> Result<Npc, Box<dyn Error>> {
    let mut bones = npc_face_skeleton(NPC_SKELETON_SHIFT).bones().to_vec();
    let joint = bones
        .iter_mut()
        .find(|bone| bone.name == NPC_FACIAL_BONE)
        .ok_or("the facial joint")?;
    joint.rotation = axis(2, 0.7);
    let mut entries = npc_face_entries_posed_by(&TestSkeleton::new(bones));
    let logic_entry = format!("Data/{NPC_RIG_LOGIC}");
    let entry = entries
        .iter_mut()
        .find(|entry| {
            entry
                .path()
                .replace('\\', "/")
                .eq_ignore_ascii_case(&logic_entry)
        })
        .ok_or("the original logic entry")?;
    let mut logic = npc_face_logic();
    logic
        .deltas
        .as_mut()
        .ok_or("the synthetic joint records")?
        .secondary
        .insert(
            0,
            TestLogicRecord {
                output: 0,
                entries: vec![(1, 300_000)],
            },
        );
    *entry = TestEntry::stored(logic_entry, logic.build_rig());
    build(entries, &Expressions::All)
}

fn track(name: &str) -> TestTrack {
    TestTrack::new(crc32fast::hash(name.as_bytes())).positions(&[
        (0, [0.0; 3]),
        (10, [1.0, 0.0, 0.0]),
        (20, [0.0; 3]),
    ])
}

fn alignment(a: [f32; 4], b: [f32; 4]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(left, right)| left * right)
        .sum::<f32>()
        .abs()
}

fn axis(axis: usize, radians: f32) -> [f32; 4] {
    let (sine, cosine) = (radians * 0.5).sin_cos();
    let unit = |target| if target == axis { sine } else { 0.0 };
    [unit(0), unit(1), unit(2), cosine]
}

#[test]
fn authored_clip_bounds_require_an_original_rate_and_keep_only_valid_frames() -> TestResult {
    let npc = build(npc_face_entries(), &Expressions::All)?;
    let models = ArchiveModels::new(&npc.archive);
    let mut clip = animation(&TestAnimation::new("face.caf", 30).track(track("CTL_b")))?;
    clip.end_frame = 17;
    let comparison = npc.armour.verify_face_rotations(&clip, &models, &[0, 20])?;
    let header = comparison
        .verification_records()
        .next()
        .ok_or("source timeline")?;
    assert_eq!(
        header.pointer("/clip"),
        Some(
            &json!({"name": clip.name, "path": clip.path, "framesPerSecond": 30,
            "declaredEndFrame": 17, "lastKeyFrame": 20, "firstFrame": 0, "endFrame": 20})
        ),
        "authored last key extends the declared duration"
    );
    let error = npc
        .armour
        .verify_face_rotations(&clip, &models, &[21])
        .err();
    assert!(
        error.is_some_and(|message| message.contains("outside 0..=20")),
        "an out-of-range frame cannot be replaced by a held value"
    );
    clip.frames_per_second = 0;
    let error = npc.armour.verify_face_rotations(&clip, &models, &[0]).err();
    assert!(
        error.is_some_and(|message| message.contains("frame rate is unavailable")),
        "verification cannot invent a frame rate"
    );
    clip.frames_per_second = 30;
    clip.tracks.clear();
    let error = npc.armour.verify_face_rotations(&clip, &models, &[0]).err();
    assert!(
        error.is_some_and(|message| message.contains("no authored key bounds")),
        "a declared end alone cannot replace missing source tracks"
    );
    Ok(())
}

#[test]
fn commuting_candidates_do_not_select_an_engine_convention() -> TestResult {
    let npc = build(npc_face_entries(), &Expressions::All)?;
    let models = ArchiveModels::new(&npc.archive);
    let clip = animation(&TestAnimation::new("face.caf", 30).track(track("CTL_b")))?;
    let comparison = npc
        .armour
        .verify_face_rotations(&clip, &models, &[0, 10, 20])?;
    let candidates = comparison.animations().sampled();
    let first = candidates.first().ok_or("one candidate")?;
    assert_eq!(candidates.len(), 4, "one candidate for each convention");
    assert!(
        !first.bones.is_empty()
            && candidates
                .iter()
                .all(|other| other.bones == first.bones && other.weights == first.weights),
        "an actual single-axis movement commutes with the identity bind"
    );
    let header = comparison
        .verification_records()
        .next()
        .ok_or("comparison header")?;
    assert_eq!(
        header.get("selectedConvention"),
        Some(&json!(null)),
        "no selected engine convention"
    );
    assert_eq!(
        header.get("referenceComparison"),
        Some(&json!("notPerformed")),
        "producing four actions is not a reference comparison"
    );
    Ok(())
}
