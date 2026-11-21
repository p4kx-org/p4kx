// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Listing the animations next to a model from their headers.

use std::error::Error;

use p4kx_extraction::{
    AnimationChoice, AnimationListing, AnimationSet, Archive, ArchivePositionedAnimation,
    ConversionError, PathIndex,
};
use p4kx_testkit::{ArchiveBuilder, TestAnimation, TestDba, TestEntry, TestSkeleton, TestTrack};

type TestResult = Result<(), Box<dyn Error>>;

const MODEL: &str = "Data\\Objects\\door.cga";
const IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

fn animation(name: &str) -> TestAnimation {
    TestAnimation::new(name, 30).track(
        TestTrack::new(TestSkeleton::controller_id(1))
            .rotations(&[(0, IDENTITY), (30, IDENTITY)])
            .positions(&[(0, [0.0; 3]), (30, [1.0; 3])]),
    )
}

fn unknown_format() -> TestAnimation {
    TestAnimation::new("bad.caf", 30).track(
        TestTrack::new(1)
            .rotation_code(0x99)
            .rotations(&[(0, IDENTITY)]),
    )
}

fn archive(dir: &tempfile::TempDir) -> Result<Archive, Box<dyn Error>> {
    let path = dir.path().join("Data.p4k");
    let pack = TestDba::new()
        .animation(animation("pack/open.caf"))
        .animation(animation("pack/close.caf").additive());
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data\\Objects\\open.caf",
            animation("open.caf").caf(),
        ))
        .entry(TestEntry::stored(
            "Data\\Objects\\lift_add.caf",
            animation("lift_add.caf").additive().caf(),
        ))
        .entry(TestEntry::stored("Data\\Objects\\pack.dba", pack.bytes()))
        .entry(TestEntry::stored(
            "Data\\Objects\\broken.caf",
            vec![1, 2, 3],
        ))
        .entry(TestEntry::stored(
            "Data\\Other\\badkeys.caf",
            unknown_format().caf(),
        ))
        .entry(TestEntry::stored(
            "Data\\Other\\badpack.dba",
            TestDba::new()
                .animation(unknown_format())
                .animation(animation("good.caf"))
                .bytes(),
        ))
        .entry(TestEntry::stored(MODEL, vec![0; 4]))
        .write_to(&path)?;
    Ok(Archive::open_with_key(
        &path,
        Some(p4kx_testkit::test_key()),
    )?)
}

#[test]
fn the_listing_finds_what_reading_in_full_finds() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(&dir)?;
    let index = PathIndex::new(&archive);

    let listing = AnimationListing::from_archive(&index, MODEL, &[])?;
    let set = AnimationSet::from_archive(&index, MODEL, &AnimationChoice::All, &[])?;

    assert_eq!(listing.files, set.files, "files");
    assert_eq!(listing.origins, set.origins, "origins");
    assert_eq!(listing.unread, set.unread, "additive and unreadable ones");
    assert_eq!(listing.animations.len(), set.animations.len(), "count");
    for (header, full) in listing.animations.iter().zip(&set.animations) {
        assert_eq!(header.name, full.name, "name");
        assert_eq!(header.keys, full.key_count(), "keys of {}", full.name);
        assert_eq!(
            header.end_frame, full.end_frame,
            "end frame of {}",
            full.name
        );
        assert_eq!(header.controller_ids.len(), full.tracks.len(), "tracks");
    }
    Ok(())
}

#[test]
fn the_listing_reports_an_additive_animation_and_a_broken_file_as_unread() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(&dir)?;
    let index = PathIndex::new(&archive);

    let listing = AnimationListing::from_archive(&index, MODEL, &[])?;

    let names: Vec<&str> = listing
        .animations
        .iter()
        .map(|header| header.name.as_str())
        .collect();
    assert_eq!(names, ["open", "open"], "the plain ones");
    let unread: Vec<&str> = listing
        .unread
        .iter()
        .map(|unread| unread.name.as_deref().unwrap_or(unread.source.as_str()))
        .collect();
    assert!(
        unread.contains(&"close"),
        "the additive one of the database: {unread:?}"
    );
    assert!(
        unread.contains(&"lift_add"),
        "the additive file: {unread:?}"
    );
    assert!(
        unread.contains(&"Data\\Objects\\broken.caf"),
        "the broken file: {unread:?}"
    );
    Ok(())
}

#[test]
fn a_source_that_does_not_exist_is_an_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(&dir)?;
    let index = PathIndex::new(&archive);

    let result = AnimationListing::from_archive(&index, MODEL, &["Data\\none.caf".to_owned()]);

    assert!(
        matches!(result, Err(ConversionError::UnknownEntry(_))),
        "{result:?}"
    );
    Ok(())
}

#[test]
fn a_caf_named_as_a_source_is_decoded_and_a_refused_key_format_is_an_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(&dir)?;
    let index = PathIndex::new(&archive);

    let result =
        AnimationListing::from_archive(&index, MODEL, &["Data\\Other\\badkeys.caf".to_owned()]);

    assert!(
        matches!(result, Err(ConversionError::Animation { .. })),
        "{result:?}"
    );
    Ok(())
}

#[test]
fn a_database_named_as_a_source_is_decoded_and_a_refused_animation_is_unread() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(&dir)?;
    let index = PathIndex::new(&archive);

    let listing =
        AnimationListing::from_archive(&index, MODEL, &["Data\\Other\\badpack.dba".to_owned()])?;

    let names: Vec<&str> = listing
        .animations
        .iter()
        .filter(|header| header.name == "good" || header.name == "bad")
        .map(|header| header.name.as_str())
        .collect();
    assert_eq!(names, ["good"], "the readable one is listed");
    assert!(
        listing
            .unread
            .iter()
            .any(|unread| unread.name.as_deref() == Some("bad")),
        "the refused one is unread: {:?}",
        listing.unread
    );
    Ok(())
}

#[test]
fn the_selected_physical_animation_keeps_its_clip_and_controller_ordinals() -> TestResult {
    let dir = tempfile::tempdir()?;
    for caf_first in [false, true] {
        let path = dir.path().join("Data.p4k");
        let caf = if caf_first {
            "Data\\Animations\\a\\extend.caf"
        } else {
            "Data\\Animations\\z\\extend.caf"
        };
        let dba = "Data\\Animations\\m\\pack.dba";
        let clip = || {
            TestAnimation::new("extend.caf", 30)
                .track(TestTrack::new(17))
                .track(TestTrack::new(17).positions(&[(0, [0.0; 3]), (30, [3.0; 3])]))
        };
        let database = TestDba::new()
            .animation(unknown_format())
            .animation(clip().additive())
            .animation(clip());
        ArchiveBuilder::new()
            .entry(TestEntry::stored(caf, clip().caf()))
            .entry(TestEntry::stored(dba, database.bytes()).encrypted())
            .write_to(&path)?;
        let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
        let index = PathIndex::new(&archive);
        let extra = [dba.to_owned(), caf.to_owned()];
        let observed =
            ArchivePositionedAnimation::read_first_non_additive(&index, "extend.caf", &extra)?
                .ok_or("the admitted clip")?;
        assert_positioned(&index, &observed, &extra, if caf_first { caf } else { dba })?;
        assert_eq!(
            observed.animation().clip_ordinal(),
            if caf_first { 0 } else { 2 },
            "physical clip ordinal includes skipped and losing clips"
        );
    }
    Ok(())
}

fn assert_positioned(
    index: &PathIndex<'_>,
    observed: &ArchivePositionedAnimation,
    extra: &[String],
    chosen: &str,
) -> TestResult {
    let actual = index
        .find(chosen)
        .and_then(|entry| index.archive().entry_origin(entry))
        .ok_or("the original selected physical Entry")?;
    assert_eq!(
        observed
            .origin()
            .ok_or("the selected Entry origin")?
            .binding(),
        actual.binding(),
        "only the actual selected Archive Entry supplies the clip"
    );
    let choice = AnimationChoice::Named(vec!["extend.caf".to_owned()]);
    let baseline = AnimationSet::from_folders(index, &[], &choice, extra)?
        .animations
        .into_iter()
        .find(|animation| !animation.additive)
        .ok_or("normal selected animation")?;
    let positioned = observed.animation();
    assert_eq!(
        positioned.animation(),
        &baseline,
        "the ordinary decoder result"
    );
    assert_eq!(
        positioned.controller_ordinal(0),
        Some(1),
        "the admitted track retains the original controller after an empty one"
    );
    assert_eq!(
        positioned.controller_ordinal(1),
        None,
        "no decoded second track"
    );
    Ok(())
}
