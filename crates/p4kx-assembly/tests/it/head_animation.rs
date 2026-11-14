// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Animations of a Character: keys that move the bones of the body, and facial animations
//! evaluated through the rig logic into morph weights.

use std::error::Error;

use p4kx_assembly::{
    ArchiveModels, Armour, AssemblyOptions, BodyVariant, Expressions, Issue, IssueKind, Piece,
};
use p4kx_chunks::ChunkFile;
use p4kx_extraction::{Archive, SourceEncryption};
use p4kx_gltf::GlbOptions;
use p4kx_model::Animation;
use p4kx_testkit::{
    HEAD_VERTICES, MALE_RIG_HASH, TestAnimation, TestSkeleton, TestTrack, check_glb, head_entries,
    head_rig, test_key,
};

use crate::assembly_support::{Fixture, TestResult, fixture};
use crate::head_build::{dna_of_head_two_in_region_three, head_path};

fn animation(source: &TestAnimation) -> Result<Animation, Box<dyn Error>> {
    let bytes = source.caf();
    Ok(Animation::read_caf(
        source.name(),
        &ChunkFile::parse(bytes.as_slice())?,
    )?)
}

fn face_animation() -> TestAnimation {
    TestAnimation::new("blink.caf", 30).track(TestTrack::new(crc32fast::hash(b"CTL_a")).positions(
        &[
            (0, [0.0, 0.0, 0.0]),
            (10, [1.0, 0.0, 0.0]),
            (20, [0.0, 0.0, 0.0]),
        ],
    ))
}

fn body_animation() -> TestAnimation {
    TestAnimation::new("nod.caf", 30).track(
        TestTrack::new(TestSkeleton::controller_id(2))
            .rotations(&[(0, [0.0, 0.0, 0.0, 1.0]), (10, [0.0, 0.0, 0.0, 1.0])]),
    )
}

fn is_dna(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("dna"))
}

fn build() -> Result<(Armour, Archive, Fixture), Box<dyn Error>> {
    build_with_rig_encryption(false)
}

fn build_with_rig_encryption(
    encrypted: bool,
) -> Result<(Armour, Archive, Fixture), Box<dyn Error>> {
    let entries = head_entries(Some(head_rig(HEAD_VERTICES, MALE_RIG_HASH).build()))
        .into_iter()
        .map(|entry| {
            if encrypted && is_dna(entry.path()) {
                entry.encrypted()
            } else {
                entry
            }
        })
        .collect();
    let fixture = fixture(entries)?;
    let archive = Archive::open_with_key(&fixture.path, encrypted.then(test_key))?;
    let piece = Piece::file("head", head_path(), None)
        .shaped(dna_of_head_two_in_region_three())
        .with_expressions(Expressions::All);
    let armour = Armour::build(
        BodyVariant::Male,
        &[piece],
        &ArchiveModels::new(&archive),
        &AssemblyOptions::default(),
    )?;
    Ok((armour, archive, fixture))
}

#[test]
fn a_facial_animation_sets_the_weights_of_the_morph_targets_and_a_body_animation_moves_bones()
-> TestResult {
    let (armour, archive, _dir) = build()?;
    let models = ArchiveModels::new(&archive);
    let list = [animation(&face_animation())?, animation(&body_animation())?];

    let animations = armour.animate(&list, &models);

    assert!(animations.issues().is_empty(), "{:?}", animations.issues());
    let [face] = animations.sampled() else {
        return Err("one facial animation".into());
    };
    assert_eq!(face.times, [0.0, 10.0 / 30.0, 20.0 / 30.0]);
    let weights = &face.weights.first().ok_or("weights")?.values;
    assert_eq!(weights, &[0.0, 0.0, 1.0, 0.0, 0.0, 0.0]);

    let mut bytes = Vec::new();
    let summary = armour.write_glb_animated(&animations, &GlbOptions::default(), &mut bytes)?;
    let glb = check_glb(&bytes)?;
    assert_eq!(glb.document.animations().count(), 2);
    assert_eq!(summary.animations.len(), 2);
    Ok(())
}

#[test]
fn an_animation_that_fits_nothing_is_listed_and_not_written() -> TestResult {
    let (armour, archive, _dir) = build()?;
    let models = ArchiveModels::new(&archive);
    let stranger = TestAnimation::new("stranger.caf", 30)
        .track(TestTrack::new(12345).positions(&[(0, [0.0; 3]), (5, [1.0, 0.0, 0.0])]));

    let list = [animation(&stranger)?];
    let animations = armour.animate(&list, &models);

    assert!(animations.is_empty());
    let codes: Vec<&str> = animations
        .issues()
        .iter()
        .map(|issue| issue.kind().code())
        .collect();
    assert_eq!(codes, ["animation_not_used"]);
    assert!(matches!(
        animations.issues().first().map(Issue::kind),
        Some(IssueKind::AnimationNotUsed { .. })
    ));
    Ok(())
}

#[test]
fn many_unused_animations_are_one_issue_with_a_count_and_a_few_names() -> TestResult {
    let (armour, archive, _dir) = build()?;
    let models = ArchiveModels::new(&archive);
    let stranger = animation(
        &TestAnimation::new("stranger.caf", 30)
            .track(TestTrack::new(12345).positions(&[(0, [0.0; 3]), (5, [1.0, 0.0, 0.0])])),
    )?;
    let list: Vec<Animation> = (0..2_000).map(|_| stranger.clone()).collect();

    let animations = armour.animate(&list, &models);

    let [issue] = animations.issues() else {
        return Err("one issue for all animations".into());
    };
    assert!(
        matches!(
            issue.kind(),
            IssueKind::AnimationNotUsed { count: 2_000, examples, .. } if examples.len() == 3
        ),
        "{issue:?}"
    );
    assert!(issue.kind().to_string().starts_with("2000 animations"));
    assert!(issue.kind().to_string().len() < 300);
    assert_eq!(issue.name(), "animations");
    Ok(())
}

#[test]
fn a_clip_with_bone_and_face_tracks_over_the_budget_is_counted_once() -> TestResult {
    let (armour, archive, _dir) = build()?;
    let models = ArchiveModels::new(&archive);
    let both = TestAnimation::new("both.caf", 30)
        .track(
            TestTrack::new(TestSkeleton::controller_id(2))
                .rotations(&[(0, [0.0, 0.0, 0.0, 1.0]), (10, [0.0, 0.0, 0.0, 1.0])]),
        )
        .track(TestTrack::new(crc32fast::hash(b"CTL_a")).positions(&[
            (0, [0.0, 0.0, 0.0]),
            (10, [1.0, 0.0, 0.0]),
            (20, [0.0, 0.0, 0.0]),
        ]));
    let list = [animation(&both)?];

    let animations = armour.animate_within(&list, &models, 5);

    assert!(animations.is_empty());
    let [issue] = animations.issues() else {
        return Err(format!("one issue: {:?}", animations.issues()).into());
    };
    assert!(
        matches!(issue.kind(), IssueKind::AnimationNotUsed { count: 1, .. }),
        "{issue:?}"
    );
    assert_eq!(issue.name(), "both");
    Ok(())
}

#[test]
fn the_facial_joints_of_the_skin_join_the_skeleton() -> TestResult {
    let (armour, _archive, _dir) = build()?;

    assert_eq!(
        armour.skeleton().bones.len(),
        3,
        "the fixture's head has the body's bones"
    );
    assert!(armour.issues().is_empty(), "{:?}", armour.issues());
    Ok(())
}

fn reason_of(issue: &Issue) -> &str {
    match issue.kind() {
        IssueKind::AnimationNotUsed { reason, .. } => reason,
        _ => "",
    }
}

#[test]
fn animations_over_the_budget_are_listed_with_the_reason_limit_and_not_written() -> TestResult {
    let (armour, archive, _dir) = build()?;
    let models = ArchiveModels::new(&archive);
    let list = [animation(&face_animation())?, animation(&body_animation())?];

    let animations = armour.animate_within(&list, &models, 5);

    assert!(animations.is_empty());
    assert_eq!(animations.values(), 0);
    let [issue] = animations.issues() else {
        return Err("one issue for both animations".into());
    };
    assert!(reason_of(issue).starts_with("limit"), "{issue:?}");
    assert!(
        matches!(
            issue.kind(),
            IssueKind::AnimationNotUsed { count: 2, examples, .. } if examples == &["blink", "nod"]
        ),
        "{issue:?}"
    );
    let spent = armour.animate_within(&list, &models, 10);
    assert_eq!(spent.len(), 1, "the body animation fits exactly");
    assert_eq!(spent.values(), 10);
    Ok(())
}

#[test]
fn control_values_are_held_to_zero_and_one() -> TestResult {
    let (armour, archive, _dir) = build()?;
    let models = ArchiveModels::new(&archive);
    let wild = TestAnimation::new("wild.caf", 30).track(
        TestTrack::new(crc32fast::hash(b"CTL_a")).positions(&[
            (0, [0.0, 0.0, 0.0]),
            (10, [1e30, 0.0, 0.0]),
            (20, [-1e30, 0.0, 0.0]),
        ]),
    );
    let list = [animation(&wild)?];

    let animations = armour.animate(&list, &models);

    assert!(animations.issues().is_empty(), "{:?}", animations.issues());
    let weights = &animations.sampled()[0].weights[0].values;
    assert!(
        weights.iter().all(|weight| (0.0..=1.0).contains(weight)),
        "{weights:?}"
    );
    assert_eq!(weights[2], 1.0);
    Ok(())
}

#[test]
fn shaping_and_the_emitted_face_animation_retain_their_actual_encrypted_rig_reads() -> TestResult {
    let (armour, archive, _directory) = build_with_rig_encryption(true)?;
    let models = ArchiveModels::new(&archive);
    let head = armour
        .parts()
        .iter()
        .find(|part| part.name() == "head")
        .ok_or("head part")?;
    let [shape] = head.shape_archive_sources() else {
        return Err("one head shape source".into());
    };
    let rig = shape.rig.entry.as_ref().ok_or("shaping rig Entry")?;
    assert_eq!(rig.encryption(), SourceEncryption::Encrypted);
    assert!(is_dna(rig.entry().path()));
    assert!(head.has_facial_rig());

    let list = [animation(&face_animation())?, animation(&body_animation())?];
    let animated = armour.animate(&list, &models);
    assert!(animated.rig_read_for(0).is_none());
    let sampled = animated
        .rig_read_for(1)
        .and_then(|rig| rig.entry.as_ref())
        .ok_or("sampled rig Entry")?;
    assert_eq!(sampled.entry().path(), rig.entry().path());
    assert_eq!(sampled.encryption(), SourceEncryption::Encrypted);
    let mut glb = Vec::new();
    let summary = armour.write_glb_animated(&animated, &GlbOptions::default(), &mut glb)?;
    assert_eq!(summary.animations.len(), 2);
    assert!(summary.animations[1].channels > 0);
    Ok(())
}
