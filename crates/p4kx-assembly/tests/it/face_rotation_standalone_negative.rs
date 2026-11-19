// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::ArchiveModels;
use p4kx_chunks::ChunkFile;
use p4kx_model::AnimationSource;
use p4kx_testkit::{TestAnimation, TestRotationFormat, TestTrack};

use super::{IDENTITY, TestResult, neutral, npc, scalar};

fn refused(source: &TestAnimation, reason: &str) -> TestResult {
    let npc = npc()?;
    let models = ArchiveModels::new(&npc.archive);
    let error = npc
        .armour
        .verify_standalone_face_rotations_from_caf("face.caf", &source.caf(), &models, &[0, 10, 20])
        .err()
        .ok_or("unsupported source must remain refused")?;
    assert!(
        error.contains(reason),
        "expected {reason:?}, observed {error:?}"
    );
    Ok(())
}

#[test]
fn active_and_nonunit_scalar_less_rotations_are_not_neutralized() -> TestResult {
    let (sine, cosine) = 0.25_f32.sin_cos();
    let active = TestTrack::new(crc32fast::hash(b"CTL_b"))
        .rotation_format(TestRotationFormat::Float)
        .rotations(&[
            (0, IDENTITY),
            (10, [sine, 0.0, 0.0, cosine]),
            (20, IDENTITY),
        ]);
    refused(
        &TestAnimation::new("face.caf", 30)
            .track(scalar("CTL_a"))
            .track(active),
        "active or noncanonical",
    )?;
    let nonunit = TestTrack::new(crc32fast::hash(b"CTL_b"))
        .rotation_format(TestRotationFormat::Float)
        .rotations(&[(0, [0.0, 0.0, 0.0, 2.0]), (20, [0.0, 0.0, 0.0, 2.0])]);
    refused(
        &TestAnimation::new("face.caf", 30)
            .track(scalar("CTL_a"))
            .track(nonunit),
        "active or noncanonical",
    )
}

#[test]
fn a_near_identity_encoded_mutation_cannot_reuse_a_previous_verdict() -> TestResult {
    let npc = npc()?;
    let models = ArchiveModels::new(&npc.archive);
    let mut bytes = TestAnimation::new("face.caf", 30)
        .track(scalar("CTL_a"))
        .track(neutral("CTL_b"))
        .caf();
    let offset = {
        let file = ChunkFile::parse(bytes.as_slice())?;
        let source = AnimationSource::read_caf("face.caf", &file)?;
        let original = source
            .controllers()?
            .find(|row| row.controller_id() == crc32fast::hash(b"CTL_b"))
            .ok_or("original neutral control")?;
        let evidence = original
            .rotation_identity()?
            .ok_or("authored identity keys")?;
        assert!(
            evidence.is_canonical(),
            "source initially carries the exact identity code"
        );
        usize::try_from(evidence.value_range().start)?
    };
    *bytes.get_mut(offset).ok_or("original encoded component")? ^= 1;
    let error = npc
        .armour
        .verify_standalone_face_rotations_from_caf("face.caf", &bytes, &models, &[0, 10, 20])
        .err()
        .ok_or("changed source must be revalidated internally")?;
    assert!(
        error.contains("active or noncanonical"),
        "same-input ownership prevents a stale proof from admitting changed bytes"
    );
    Ok(())
}

#[test]
fn unknown_and_repeated_source_targets_remain_explicit_failures() -> TestResult {
    refused(
        &TestAnimation::new("face.caf", 30)
            .track(scalar("CTL_a"))
            .track(neutral("unknown-synthetic")),
        "unknown rig control",
    )?;
    refused(
        &TestAnimation::new("face.caf", 30)
            .track(scalar("CTL_a"))
            .track(neutral("CTL_b"))
            .track(neutral("CTL_b")),
        "repeated original records",
    )?;
    refused(
        &TestAnimation::new("face.caf", 30)
            .track(scalar("CTL_a"))
            .track(neutral("CTL_b"))
            .track(scalar("CTL_b")),
        "repeated original records",
    )
}

#[test]
fn a_default_only_sequence_does_not_become_a_complete_animation() -> TestResult {
    refused(
        &TestAnimation::new("face.caf", 30)
            .track(neutral("CTL_b"))
            .track(TestTrack::new(crc32fast::hash(b"CTL_PSD"))),
        "keyed nonconstant",
    )
}

#[test]
fn additive_and_skeletal_payload_keep_the_original_strict_admission() -> TestResult {
    refused(
        &TestAnimation::new("face.caf", 30)
            .additive()
            .track(scalar("CTL_a")),
        "absolute facial-only",
    )?;
    let npc = npc()?;
    let bone = npc
        .armour
        .skeleton()
        .bones
        .first()
        .ok_or("original skeleton bone")?
        .controller_id;
    let source = TestAnimation::new("face.caf", 30)
        .track(scalar("CTL_a"))
        .track(TestTrack::new(bone).positions(&[(0, [0.0; 3]), (20, [0.0; 3])]));
    let models = ArchiveModels::new(&npc.archive);
    let error = npc
        .armour
        .verify_standalone_face_rotations_from_caf("face.caf", &source.caf(), &models, &[0, 10, 20])
        .err()
        .ok_or("mixed skeletal source")?;
    assert!(
        error.contains("absolute facial-only"),
        "a modeled scalar default cannot absorb skeletal animation"
    );
    Ok(())
}
