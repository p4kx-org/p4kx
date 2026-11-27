// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "face_rotation_standalone_negative.rs"]
mod negative;

use std::error::Error;

use p4kx_assembly::{ArchiveModels, Expressions, FaceRotationComparison};
use p4kx_chunks::ChunkFile;
use p4kx_model::AnimationSource;
use p4kx_testkit::{TestAnimation, TestChunkFile, TestTrack, npc_face_entries};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{Npc, animation, build};

type TestResult = Result<(), Box<dyn Error>>;
const IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

fn npc() -> Result<Npc, Box<dyn Error>> {
    build(npc_face_entries(), &Expressions::All)
}

fn scalar(name: &str) -> TestTrack {
    TestTrack::new(crc32fast::hash(name.as_bytes())).positions(&[
        (0, [0.0; 3]),
        (10, [1.0, 0.0, 0.0]),
        (20, [0.0; 3]),
    ])
}

fn neutral(name: &str) -> TestTrack {
    TestTrack::new(crc32fast::hash(name.as_bytes())).rotations(&[
        (0, IDENTITY),
        (6, IDENTITY),
        (13, IDENTITY),
        (20, IDENTITY),
    ])
}

fn compare(first: &FaceRotationComparison, second: &FaceRotationComparison) {
    assert_eq!(
        first.angles_radians(),
        second.angles_radians(),
        "same original modeled Euler values"
    );
    let first = first.animations().sampled();
    let second = second.animations().sampled();
    assert_eq!(first.len(), 4, "four strict candidates");
    assert_eq!(second.len(), first.len(), "all standalone candidates");
    for (left, right) in first.iter().zip(second) {
        assert_eq!(left.name, right.name, "same convention");
        assert_eq!(left.times, right.times, "same source clock");
        assert_eq!(left.bones, right.bones, "same joint poses");
        assert_eq!(left.weights, right.weights, "same morph activations");
        assert!(
            right.skipped_tracks.is_empty(),
            "only proved neutral placeholders are removed from the scalar view"
        );
    }
}

fn control<'a>(rows: &'a [Value], name: &str) -> Result<&'a Value, Box<dyn Error>> {
    let id = crc32fast::hash(name.as_bytes());
    let mut matching = rows.iter().filter(|row| {
        row.get("record") == Some(&json!("standaloneFaceControl"))
            && row.get("controllerId").and_then(Value::as_u64) == Some(u64::from(id))
    });
    let found = matching.next().ok_or("original source control")?;
    assert!(
        matching.next().is_none(),
        "one original target per synthetic control"
    );
    Ok(found)
}

fn check_original_rows(rows: &[Value], bytes: &[u8]) -> TestResult {
    let profile = rows.first().ok_or("standalone profile")?;
    let file = ChunkFile::parse(bytes)?;
    let source = AnimationSource::read_caf("face.caf", &file)?;
    let original = source
        .controllers()?
        .find(|row| row.controller_id() == crc32fast::hash(b"CTL_b"))
        .ok_or("the original placeholder")?;
    let detail = control(rows, "CTL_b")?;
    assert_eq!(
        detail.get("state"),
        Some(&json!("modeledNeutral")),
        "no authored zero is invented"
    );
    assert_eq!(
        detail.get("modeledValue"),
        Some(&json!(0.0)),
        "ordinary unkeyed control default"
    );
    assert_eq!(
        detail.get("controllerOrdinal"),
        Some(&json!(original.ordinal())),
        "physical source occurrence retained"
    );
    let descriptor = original.table_range();
    assert_eq!(
        detail.pointer("/sourceBytes/controllerDescriptor"),
        Some(&json!([descriptor.start, descriptor.end])),
        "source descriptor survives the scalar view"
    );
    let rotation = original.decode_rotation()?.ok_or("source rotation span")?;
    assert_eq!(
        detail.pointer("/sourceBytes/rotationValues"),
        Some(&json!([
            rotation.value_range.start,
            rotation.value_range.end
        ])),
        "actual encoded identity bytes"
    );
    let empty = control(rows, "CTL_PSD")?;
    assert_eq!(
        empty.get("decodedTrack"),
        Some(&json!(null)),
        "empty source controller was not materialized as a track"
    );
    assert_eq!(
        empty.get("modeledValue"),
        Some(&json!(1.0)),
        "constant slot0 is modeled as1"
    );
    assert_eq!(
        profile.get("sourceControllerOccurrences"),
        Some(&json!(source.controllers()?.count())),
        "empty authored rows are accounted for"
    );
    Ok(())
}

#[test]
fn source_owned_neutral_controls_preserve_all_candidates_and_original_occurrences() -> TestResult {
    let npc = npc()?;
    let models = ArchiveModels::new(&npc.archive);
    let active = TestAnimation::new("face.caf", 30).track(scalar("CTL_a"));
    let original = animation(&active)?;
    let strict = npc
        .armour
        .verify_face_rotations(&original, &models, &[0, 10, 20])?;
    let with_neutral = TestAnimation::new("face.caf", 30)
        .track(scalar("CTL_a"))
        .track(neutral("CTL_b"))
        .track(TestTrack::new(crc32fast::hash(b"CTL_PSD")));
    let bytes = with_neutral.caf();
    assert!(
        npc.armour
            .verify_face_rotations(&animation(&with_neutral)?, &models, &[0, 10, 20])
            .is_err(),
        "the strict API still refuses a scalar-less placeholder"
    );
    let standalone = npc.armour.verify_standalone_face_rotations_from_caf(
        "face.caf",
        &bytes,
        &models,
        &[0, 10, 20],
    )?;
    compare(&strict, standalone.comparison());
    let rows: Vec<_> = standalone.verification_records().collect();
    let profile = rows.first().ok_or("standalone profile")?;
    check_profile_source(profile, &bytes)?;
    check_original_rows(&rows, &bytes)?;
    assert_eq!(
        strict.verification_records().collect::<Vec<_>>(),
        standalone
            .comparison()
            .verification_records()
            .collect::<Vec<_>>(),
        "strict comparison header/angle contract remains identical"
    );
    Ok(())
}

fn check_profile_source(profile: &Value, bytes: &[u8]) -> TestResult {
    assert_eq!(
        profile.get("runtimeInitialization"),
        Some(&json!("notEstablished")),
        "modeled initialization is not game runtime proof"
    );
    assert_eq!(
        profile.pointer("/source/archiveBinding"),
        Some(&json!("notEstablished")),
        "a plain slice does not gain trusted Archive origin"
    );
    let digest = profile
        .pointer("/source/sha256")
        .and_then(Value::as_str)
        .ok_or("the profile binds its coordinates to a SHA-256 digest")?;
    let expected = Sha256::digest(bytes);
    assert_eq!(digest.len(), expected.len() * 2, "two hex digits per byte");
    let (encoded_bytes, _) = digest.as_bytes().as_chunks::<2>();
    for (encoded, byte) in encoded_bytes.iter().zip(expected) {
        assert_eq!(
            u8::from_str_radix(std::str::from_utf8(encoded)?, 16)?,
            byte,
            "relative coordinates bind to the exact input slice"
        );
    }
    Ok(())
}

#[test]
fn scalar_projection_keeps_original_declared_and_last_key_bounds() -> TestResult {
    let npc = npc()?;
    let models = ArchiveModels::new(&npc.archive);
    let track = TestTrack::new(crc32fast::hash(b"CTL_b")).rotations(&[
        (0, IDENTITY),
        (6, IDENTITY),
        (13, IDENTITY),
        (40, IDENTITY),
    ]);
    let clip = TestAnimation::new("face.caf", 30)
        .track(scalar("CTL_a"))
        .track(track);
    let mut metadata = clip.anim_info();
    metadata
        .get_mut(16..20)
        .ok_or("authored end word")?
        .copy_from_slice(&7_u32.to_le_bytes());
    let bytes = TestChunkFile::ivo()
        .chunk(0x4733_C6ED, 0x901, metadata)
        .chunk(0xA949_6CB5, 0x900, clip.caf_data())
        .bytes();
    let result = npc.armour.verify_standalone_face_rotations_from_caf(
        "face.caf",
        &bytes,
        &models,
        &[0, 20, 40],
    )?;
    let header = result
        .comparison()
        .verification_records()
        .next()
        .ok_or("original bounds")?;
    assert_eq!(
        header.pointer("/clip/declaredEndFrame"),
        Some(&json!(7)),
        "authored declaration is not overwritten"
    );
    assert_eq!(
        header.pointer("/clip/lastKeyFrame"),
        Some(&json!(40)),
        "neutral rotation source still defines an original bound"
    );
    assert_eq!(
        header.pointer("/clip/endFrame"),
        Some(&json!(40)),
        "scalar projection cannot shorten the admitted source interval"
    );
    Ok(())
}

#[test]
fn sourced_constant_input_remains_a_modeled_rig_constant() -> TestResult {
    let npc = npc()?;
    let models = ArchiveModels::new(&npc.archive);
    let raw = TestAnimation::new("face.caf", 30)
        .track(scalar("CTL_a"))
        .track(scalar("CTL_PSD"))
        .caf();
    let result = npc.armour.verify_standalone_face_rotations_from_caf(
        "face.caf",
        &raw,
        &models,
        &[0, 10, 20],
    )?;
    let rows: Vec<_> = result.verification_records().collect();
    let constant = control(&rows, "CTL_PSD")?;
    assert_eq!(
        constant.get("state"),
        Some(&json!("constantInput")),
        "the authored scalar channel remains visible"
    );
    assert_eq!(
        constant.get("modeledValue"),
        Some(&json!(1.0)),
        "RigLogic still forces slot0 to1"
    );
    Ok(())
}
