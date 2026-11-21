// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use crate::complete_stinger_fixture::{self as fixture, BankSource, Case};
use p4kx_assembly::audio::MAX_SOUNDBANK_LEN;
use p4kx_audio::{HircValue, Soundbank};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestEntry};

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub(crate) fn with_archive(case: Case, check: impl FnOnce(&Archive) -> TestResult) -> TestResult {
    let banks = fixture::banks(case);
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("complete-stingers.p4k");
    let mut builder = ArchiveBuilder::new().entry(TestEntry::stored(
        "Data\\Libs\\GameAudio\\complete.xml",
        TestAtl::new("Complete")
            .trigger(fixture::TRIGGER, &[fixture::EVENT])
            .to_xml(),
    ));
    for bank in &banks {
        builder = builder.entry(TestEntry::stored(bank.path, bank.bytes.as_slice()));
    }
    builder.write_to(&path)?;
    let archive = Archive::open(path)?;
    let mut unique = HashSet::new();
    let mut authored = 0;
    for expected in &banks {
        authored += original_bank(&archive, expected, &mut unique)?;
    }
    let (expected_authored, expected_unique) = match case {
        Case::SingleBoundary | Case::GlobalBoundary => (262_145, 262_145),
        Case::Duplicates => (4, 2),
        Case::Missing => (2, 2),
        Case::Small => (3, 3),
    };
    assert_eq!(
        authored, expected_authored,
        "all genuine original source occurrences before mapping"
    );
    assert_eq!(
        unique.len(),
        expected_unique,
        "actual authored trigger/bank/segment identities"
    );
    check(&archive)
}

fn original_bank(
    archive: &Archive,
    expected: &BankSource,
    unique: &mut HashSet<(u32, u32, u32)>,
) -> TestResult<usize> {
    use std::io::Read as _;

    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == expected.path)
        .ok_or("the actual original bank Entry")?;
    assert!(
        entry.uncompressed_size() < MAX_SOUNDBANK_LEN,
        "independent bank byte admission fits"
    );
    let mut original = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut original)?;
    assert_eq!(
        original, expected.bytes,
        "every encoded source byte is actually readable"
    );
    let bank = Soundbank::parse(original.as_slice())?;
    original_header(&bank, expected);
    let holder = bank.object(fixture::HOLDER).ok_or("actual source holder")?;
    let HircValue::MusicSegment(segment) = &holder.value else {
        return Err("source music holder".into());
    };
    let expected_count = usize::try_from(expected.prefix + expected.tails)?;
    assert_eq!(
        segment.node.stingers.len(),
        expected_count,
        "all original count-declared slots decode"
    );
    for (stinger, ordinal) in segment.node.stingers.iter().zip(0_u32..) {
        let actual = (stinger.trigger_id, stinger.segment_id);
        let expected_value = if ordinal < expected.prefix {
            (fixture::COMMON, fixture::FIRST_PREFIX + ordinal)
        } else {
            (fixture::LATE, fixture::TARGET)
        };
        assert_eq!(
            actual, expected_value,
            "exact actual original slot including the late target"
        );
        unique.insert((stinger.trigger_id, bank.header().id, stinger.segment_id));
    }
    if expected.tails > 0 {
        original_target(&bank)?;
    }
    if expected.path.ends_with("Z-tail.bnk") {
        original_event(&bank)?;
    }
    Ok(segment.node.stingers.len())
}

fn original_header(bank: &Soundbank<&[u8]>, expected: &BankSource) {
    assert_eq!(
        bank.header().version,
        154,
        "the source version selects the declared layout"
    );
    assert_eq!(
        bank.header().id,
        expected.id,
        "genuine distinct source bank identity"
    );
    assert!(
        bank.warnings().is_empty(),
        "no malformed bank warning can stand in for semantic RED"
    );
}

fn original_target(bank: &Soundbank<&[u8]>) -> TestResult {
    let target = bank
        .object(fixture::TARGET)
        .ok_or("actual late target segment")?;
    let HircValue::MusicSegment(segment) = &target.value else {
        return Err("source target type".into());
    };
    assert_eq!(
        segment.node.children,
        [fixture::TRACK],
        "actual target-to-track edge"
    );
    assert!(
        segment.node.stingers.is_empty(),
        "the target adds no hidden extra stinger"
    );
    let track = bank
        .object(fixture::TRACK)
        .ok_or("actual original music track")?;
    let HircValue::MusicTrack(track) = &track.value else {
        return Err("source track type".into());
    };
    let sources: Vec<_> = track
        .sources
        .iter()
        .map(|source| source.source_id)
        .collect();
    assert_eq!(
        sources,
        [fixture::WEM],
        "the genuine target leads to the declared media ID"
    );
    Ok(())
}

fn original_event(bank: &Soundbank<&[u8]>) -> TestResult {
    let event = bank
        .object(p4kx_testkit::wwise_short_id(fixture::EVENT))
        .ok_or("actual source Event")?;
    let HircValue::Event(actions) = &event.value else {
        return Err("source Event type".into());
    };
    assert_eq!(actions, &[fixture::ACTION], "actual Event-to-action edge");
    let action = bank.object(fixture::ACTION).ok_or("actual source action")?;
    let HircValue::Action(action) = &action.value else {
        return Err("source action type".into());
    };
    assert!(action.is_trigger(), "actual Trigger action kind");
    assert_eq!(
        action.target_id,
        fixture::LATE,
        "the selected trigger is the late source fact"
    );
    Ok(())
}
