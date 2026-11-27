// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{ArchiveModels, Armour, CharacterAnimationRole, CharacterAnimations};
use p4kx_extraction::{AnimationChoice, AnimationSet, PathIndex, SourceEncryption};
use p4kx_gltf::GlbOptions;
use p4kx_testkit::{TestAnimation, TestEntry, TestSkeleton, TestTrack};

use super::{body_animation, build_with_clips, face_animation};
use crate::assembly_support::TestResult;

fn mixed_clip_entries() -> [TestEntry; 3] {
    let mixed = face_animation().track(
        TestTrack::new(TestSkeleton::controller_id(2))
            .rotations(&[(0, [0.0, 0.0, 0.0, 1.0]), (30, [0.0, 0.0, 0.0, 1.0])]),
    );
    let unused = TestAnimation::new("unused.caf", 30)
        .track(TestTrack::new(12345).positions(&[(0, [0.0; 3]), (5, [1.0, 0.0, 0.0])]));
    [
        TestEntry::stored(r"Data\Animations\Test\a-mixed.caf", mixed.caf()),
        TestEntry::stored(r"Data\Animations\Test\b-unused.caf", unused.caf()),
        TestEntry::stored(r"Data\Animations\Test\c-body.caf", body_animation().caf()),
    ]
}

#[test]
fn observed_fitting_binds_the_original_read_to_keyed_and_sampled_writer_slots() -> TestResult {
    let (armour, archive, _directory) = build_with_clips(true, mixed_clip_entries())?;
    let models = ArchiveModels::new(&archive);
    let found = AnimationSet::from_folders_with_clip_evidence(
        &PathIndex::new(&archive),
        &["Data/Animations/Test"],
        &AnimationChoice::All,
        &[],
    )?;
    let bound = armour.animate_with_clip_evidence(&found, &models);
    let plain = armour.animate(&found.set().animations, &models);
    assert!(
        bound
            .archive_animation_read()
            .is_some_and(|read| std::ptr::eq(read, &raw const found)),
        "the retained witness is the very read that supplied the fitted clips"
    );
    assert!(
        plain.archive_animation_read().is_none(),
        "plain fitting has no bound read"
    );
    compare_prepared_slots(&bound)?;
    compare_clip_emission(&armour, &bound, &plain)
}

fn compare_prepared_slots(bound: &CharacterAnimations<'_>) -> TestResult {
    assert!(
        bound.input_indices().eq([0, 2, 0]),
        "the mixed clip is reused and the unused clip is skipped"
    );
    for (writer, (source, role)) in [
        (0, CharacterAnimationRole::Keys),
        (2, CharacterAnimationRole::Keys),
        (0, CharacterAnimationRole::Sampled),
    ]
    .into_iter()
    .enumerate()
    {
        let input = bound.input_for(writer).ok_or("missing prepared slot")?;
        assert_eq!(
            input.index(),
            source,
            "original index at writer slot {writer}"
        );
        assert_eq!(input.role(), role, "role at writer slot {writer}");
    }
    assert!(
        bound.input_for(bound.len()).is_none(),
        "past the prepared slots"
    );
    assert!(
        bound.rig_read_for(0).is_none(),
        "authored keys need no sampled rig"
    );
    let sampled_rig = bound
        .rig_read_for(2)
        .and_then(|rig| rig.entry.as_ref())
        .ok_or("missing sampled rig Entry")?;
    assert_eq!(
        sampled_rig.encryption(),
        SourceEncryption::Encrypted,
        "sampled rig witness"
    );
    Ok(())
}

fn compare_clip_emission(
    armour: &Armour,
    bound: &CharacterAnimations<'_>,
    plain: &CharacterAnimations<'_>,
) -> TestResult {
    let options = GlbOptions::default();
    let mut observed_bytes = Vec::new();
    let mut plain_bytes = Vec::new();
    let summary = armour.write_glb_animated(bound, &options, &mut observed_bytes)?;
    let plain_summary = armour.write_glb_animated(plain, &options, &mut plain_bytes)?;
    assert_eq!(summary, plain_summary, "evidence preserves the GLB summary");
    assert!(
        observed_bytes == plain_bytes,
        "evidence preserves the written GLB bytes"
    );
    let [keys, _, sampled] = summary.animations.as_slice() else {
        return Err("expected three prepared writer results".into());
    };
    assert_eq!(
        keys.name, sampled.name,
        "same authored clip appears in both roles"
    );
    assert!(
        sampled.duration_seconds < keys.duration_seconds,
        "the sampled duration is transformed"
    );
    Ok(())
}
