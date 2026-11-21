// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_assembly::{
    AnimationFolders, ArchiveModels, Armour, AssemblyOptions, BodyVariant,
    character_animations_with_archive_sources,
};
use p4kx_extraction::{Archive, GlbOptions, OpenOptions, SourceEncryption};
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
