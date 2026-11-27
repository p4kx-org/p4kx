// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_assembly::{
    AnimationFolders, ArchiveModels, Armour, AssemblyOptions, BodyVariant,
    character_animations_with_archive_sources, character_animations_with_clip_evidence,
};
use p4kx_extraction::{Archive, ConversionError, GlbOptions, OpenOptions, SourceEncryption};
use p4kx_testkit::{
    ArchiveBuilder, TestAnimation, TestDba, TestEntry, TestSkeleton, TestTrack, armour_entries,
    test_key,
};

type TestResult = Result<(), Box<dyn Error>>;

fn clip(controller: u32) -> TestAnimation {
    TestAnimation::new("same.caf", 30).track(
        TestTrack::new(controller)
            .rotations(&[(0, [0.0, 0.0, 0.0, 1.0]), (10, [0.0, 0.0, 0.0, 1.0])]),
    )
}

#[test]
fn identical_clip_names_keep_separate_archive_origins_through_the_glb_order() -> TestResult {
    const CLEAR: &str = r"Data\Animations\Test\A\same.caf";
    const UNUSED: &str = r"Data\Animations\Test\B\same.caf";
    const ENCRYPTED: &str = r"Data\Animations\Test\C\collection.dba";
    const BROKEN: &str = r"Data\Animations\Test\D\broken.caf";
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("animations.p4k");
    let mut builder = ArchiveBuilder::new();
    for entry in armour_entries() {
        builder = builder.entry(entry);
    }
    builder
        .entry(TestEntry::stored(
            CLEAR,
            clip(TestSkeleton::controller_id(2)).caf(),
        ))
        .entry(TestEntry::stored(UNUSED, clip(u32::MAX).caf()).encrypted())
        .entry(
            TestEntry::stored(
                ENCRYPTED,
                TestDba::new()
                    .animation(clip(TestSkeleton::controller_id(2)))
                    .bytes(),
            )
            .encrypted(),
        )
        .entry(TestEntry::stored(BROKEN, b"unreadable clip").encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let models = ArchiveModels::new(&archive);
    let armour = Armour::build(BodyVariant::Male, &[], &models, &AssemblyOptions::default())?;
    let found = character_animations_with_archive_sources(
        &models,
        BodyVariant::Male,
        (&["all".to_owned()], &[]),
        &AnimationFolders::only(&["Animations/Test".to_owned()]),
    )?;
    assert_eq!(found.set().origins, [CLEAR, UNUSED, ENCRYPTED]);
    assert_eq!(found.set().unread.len(), 1);
    assert_eq!(found.set().unread[0].source, BROKEN);
    let animated = armour.animate(&found.set().animations, &models);
    let mut bytes = Vec::new();
    let summary = armour.write_glb_animated(&animated, &GlbOptions::default(), &mut bytes)?;
    assert_eq!(summary.animations.len(), 2);
    assert_eq!(summary.animations[0].name, summary.animations[1].name);
    assert!(summary.animations.iter().all(|clip| clip.channels > 0));
    let sources: Vec<_> = animated
        .input_indices()
        .filter_map(|index| found.archive_source(index))
        .map(|origin| (origin.entry().path(), origin.encryption()))
        .collect();
    assert_eq!(
        sources,
        [
            (CLEAR, SourceEncryption::Unencrypted),
            (ENCRYPTED, SourceEncryption::Encrypted)
        ]
    );
    Ok(())
}

#[test]
fn positioned_character_clips_keep_npc_priority_body_fallback_and_explicit_sources() -> TestResult {
    const NPC: &str = r"Data\Animations\pu\male\fixture\same.caf";
    const BODY_SAME: &str = r"Data\Animations\Characters\Human\male_v7\same.caf";
    const BODY_FALLBACK: &str = r"Data\Animations\Characters\Human\male_v7\fallback.caf";
    const EXTRA: &str = r"Data\Fixtures\extra.caf";

    let directory = tempfile::tempdir()?;
    let path = directory.path().join("animations.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(NPC, clip(1).caf()))
        .entry(TestEntry::stored(BODY_SAME, clip(2).caf()))
        .entry(TestEntry::stored(BODY_FALLBACK, clip(3).caf()).encrypted())
        .entry(TestEntry::stored(EXTRA, clip(4).caf()))
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let models = ArchiveModels::new(&archive);
    let variant = BodyVariant::Male;
    let npc = AnimationFolders::of_npc(&models, variant, "heads/fixture_head");
    let only = AnimationFolders::only(&["Animations/pu/male/fixture".to_owned()]);
    let extras = [EXTRA.to_owned()];
    let no_extra: &[String] = &[];
    for (folders, name, extra, expected, encryption) in [
        (&npc, "same", no_extra, NPC, SourceEncryption::Unencrypted),
        (
            &npc,
            "fallback",
            no_extra,
            BODY_FALLBACK,
            SourceEncryption::Encrypted,
        ),
        (
            &only,
            "extra",
            extras.as_slice(),
            EXTRA,
            SourceEncryption::Unencrypted,
        ),
    ] {
        compare_positioned_clip(
            &models,
            variant,
            folders,
            (&[name.to_owned()], extra),
            expected,
            encryption,
        )?;
    }
    let fallback = ["fallback".to_owned()];
    assert!(
        matches!(
            character_animations_with_clip_evidence(&models, variant, (&fallback, &[]), &only),
            Err(ConversionError::UnknownAnimations { .. })
        ),
        "an explicit folder keeps its no-fallback behavior"
    );
    Ok(())
}

fn compare_positioned_clip(
    models: &ArchiveModels<'_>,
    variant: BodyVariant,
    folders: &AnimationFolders,
    requested: (&[String], &[String]),
    expected: &str,
    encryption: SourceEncryption,
) -> TestResult {
    let original = character_animations_with_archive_sources(models, variant, requested, folders)?;
    let positioned = character_animations_with_clip_evidence(models, variant, requested, folders)?;
    assert_eq!(positioned.set(), original.set(), "selection of {expected}");
    assert_eq!(positioned.set().origins, [expected], "selected file");
    let source = positioned
        .archive_source(0)
        .ok_or("missing physical clip entry")?;
    assert_eq!(source.entry().path(), expected, "source Entry");
    assert_eq!(source.encryption(), encryption, "source encryption");
    let clip = positioned.clip_evidence(0).ok_or("missing clip evidence")?;
    let coordinates = clip
        .coordinates
        .as_ref()
        .ok_or("missing clip coordinates")?;
    assert_eq!(clip.ordinal, 0, "original CAF ordinal");
    assert!(!coordinates.flags().is_empty(), "original CAF flags");
    Ok(())
}
