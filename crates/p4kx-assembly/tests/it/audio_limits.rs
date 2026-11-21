// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Source Entry and trigger limits, with complete admitted ATL event lists.

use std::error::Error;
use std::fmt::Write as _;

use p4kx_assembly::audio::{
    AudioFileError, AudioIndex, MAX_ATL_LEN, MAX_AUDIO_TRIGGERS, MAX_SOUNDBANK_LEN, WwiseEvent,
    wwise_id,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestEntry};

type TestResult = Result<(), Box<dyn Error>>;

fn index_of(entries: Vec<TestEntry>) -> Result<AudioIndex, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(AudioIndex::build(&Archive::open_with_key(
        &path,
        Some(p4kx_testkit::test_key()),
    )?))
}

fn too_large(limit: u64) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(vec![b' '; usize::try_from(limit)? + 1])
}

#[test]
fn non_ascii_names_hash_their_utf8_bytes_unfolded() {
    assert_eq!(wwise_id("Ä"), 0xCE77_C1D0, "FNV-1 over C3 84");
    assert_ne!(wwise_id("Ä"), wwise_id("ä"), "only ASCII is folded");
}

#[test]
fn entries_above_their_kind_s_limit_are_too_large() -> TestResult {
    let index = index_of(vec![
        TestEntry::stored("Data\\Libs\\GameAudio\\Huge.xml", too_large(MAX_ATL_LEN)?),
        TestEntry::stored(
            "Data\\Sounds\\wwise\\Huge.bnk",
            too_large(MAX_SOUNDBANK_LEN)?,
        ),
    ])?;
    let issues: Vec<(&str, &AudioFileError)> = index
        .issues()
        .iter()
        .map(|issue| (issue.path(), issue.error()))
        .collect();
    assert_eq!(
        issues,
        [
            (
                "Data\\Libs\\GameAudio\\Huge.xml",
                &AudioFileError::TooLarge {
                    size: MAX_ATL_LEN + 1,
                    limit: MAX_ATL_LEN
                }
            ),
            (
                "Data\\Sounds\\wwise\\Huge.bnk",
                &AudioFileError::TooLarge {
                    size: MAX_SOUNDBANK_LEN + 1,
                    limit: MAX_SOUNDBANK_LEN
                }
            ),
        ],
        "both refused before they are read"
    );
    Ok(())
}

fn many_triggers(prefix: &str, count: usize) -> String {
    let mut text = String::from("<ATLConfig><AudioTriggers>");
    for position in 0..count {
        let _infallible = write!(text, "<ATLTrigger atl_name=\"{prefix}{position}\"/>");
    }
    text.push_str("</AudioTriggers></ATLConfig>");
    text
}

#[test]
fn a_full_index_reports_the_definition_whose_triggers_it_leaves_out() -> TestResult {
    let half = MAX_AUDIO_TRIGGERS / 2 + 1;
    let index = index_of(vec![
        TestEntry::stored("Data\\Libs\\GameAudio\\A.xml", many_triggers("a", half)),
        TestEntry::stored("Data\\Libs\\GameAudio\\B.xml", many_triggers("b", half)),
    ])?;
    assert_eq!(index.triggers().len(), MAX_AUDIO_TRIGGERS, "full");
    let issues: Vec<(&str, &str)> = index
        .issues()
        .iter()
        .map(|issue| (issue.path(), issue.error().kind()))
        .collect();
    assert_eq!(
        issues,
        [("Data\\Libs\\GameAudio\\B.xml", "too_many_triggers")],
        "the second file is reported"
    );
    assert!(index.trigger("a0").is_some(), "the first file is complete");
    Ok(())
}

#[test]
fn a_trigger_keeps_every_event_of_its_admitted_definition() -> TestResult {
    let names: Vec<String> = (0..266)
        .map(|position| format!("Play_Event_{position}"))
        .collect();
    let events: Vec<&str> = names.iter().map(String::as_str).collect();
    let atl = TestAtl::new("SC_Many").trigger("Play_Many", &events);
    let index = index_of(vec![TestEntry::stored(
        "Data\\Libs\\GameAudio\\SC_Many.xml",
        atl.to_cryxml(),
    )])?;
    let trigger = index.trigger("Play_Many").ok_or("Play_Many")?;
    assert_eq!(
        trigger.events().len(),
        names.len(),
        "every original event is kept"
    );
    assert_eq!(
        trigger.events().first().map(WwiseEvent::name),
        Some("Play_Event_0"),
        "the original first event stays first"
    );
    assert!(
        index.issues().is_empty(),
        "the complete valid definition needs no issue"
    );
    Ok(())
}
