// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_testkit::{ArchiveBuilder, TestAtl, TestBank, TestEntry, TestHircObject, wwise_short_id};
use serde_json::json;

use super::txtp_clip_omissions::{clip_words, music_track_with_empty_automation};
use super::txtp_origin_assertions::check_plain_fields;
use super::txtp_transition_omissions::check_selector_bytes;
use super::{Fixture, PLAY, TestResult, extract, hirc_payload, pcm};

const BANK: &str = "Data/Sounds/wwise/Music.bnk";
const CODE: &str = "omit-music-track-subtrack-selection";
const TRACK: &str = "bnkHircMusicTrack";
const KIND_FIELD: &str = "musicTrackKind";
const COUNT_FIELD: &str = "musicTrackSubtrackCount";
const FILE_REPORT: &str = "/txtp/files/0";
const TWO_SUBTRACKS: u32 = 2;
const FIRST_SOURCE: u32 = 501;
const SECOND_SOURCE: u32 = 502;
const ID_BYTES: usize = size_of::<u32>();

#[test]
fn txtp_reports_random_and_sequence_subtrack_omissions_from_original_bank_words() -> TestResult {
    for (event, mode) in [("Play_RandomTrack", 1), ("Play_SequenceTrack", 2)] {
        let (fixture, bank) = fixture(event, mode, TWO_SUBTRACKS)?;
        let report = extract(&fixture, event, &["--format", "wav", "--txtp"], 0)?;
        let file = report.pointer(FILE_REPORT).ok_or("the event TXTP file")?;
        assert_eq!(
            file.get("approximations"),
            Some(&json!([CODE])),
            "zero timing alone leaves the authored selection unrepresented"
        );
        check_plain_fields(&report, CODE, TRACK, &[KIND_FIELD, COUNT_FIELD])?;
        check_selector_bytes(&report, KIND_FIELD, &bank, &[mode])?;
        check_selector_bytes(&report, COUNT_FIELD, &bank, &TWO_SUBTRACKS.to_le_bytes())?;
        let text = fs::read_to_string(fixture.out.join(format!("{event}.txtp")))?;
        let header = format!("# approximation: {CODE}");
        assert!(
            text.lines().any(|line| line == header),
            "the TXTP header agrees with JSON"
        );
        assert!(
            text.lines().any(|line| line == "group = -L2 #@layer-v"),
            "the file still layers the resolved media"
        );
    }
    Ok(())
}

#[test]
fn txtp_omits_the_subtrack_diagnostic_for_normal_and_single_subtrack_controls() -> TestResult {
    for (event, mode, count) in [
        ("Play_NormalTrack", 0, TWO_SUBTRACKS),
        ("Play_RandomOneSubtrack", 1, 1),
    ] {
        let (fixture, _) = fixture(event, mode, count)?;
        let report = extract(&fixture, event, &["--format", "wav", "--txtp"], 0)?;
        assert_eq!(
            report.pointer("/txtp/files/0/approximations"),
            Some(&json!([])),
            "{event} has no selection among alternatives"
        );
    }
    Ok(())
}

fn fixture(event: &str, mode: u8, count: u32) -> Result<(Fixture, Vec<u8>), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    let out = directory.path().join("out");
    let bank = TestBank::new(wwise_short_id("Music"))
        .version(154)
        .object(TestHircObject::event(wwise_short_id(event), &[1]))
        .object(TestHircObject::action(1, PLAY, 10))
        .object(selection_track(mode, count)?)
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Libs/GameAudio/Music.xml",
            TestAtl::new("Music").trigger(event, &[event]).to_cryxml(),
        ))
        .entry(TestEntry::stored(BANK, bank.as_slice()))
        .entry(TestEntry::stored(
            format!("Data/Sounds/wwise/Media/{FIRST_SOURCE}.wem"),
            pcm(),
        ))
        .entry(TestEntry::stored(
            format!("Data/Sounds/wwise/Media/{SECOND_SOURCE}.wem"),
            pcm(),
        ))
        .write_to(&archive)?;
    Ok((
        Fixture {
            _dir: directory,
            archive,
            out,
        },
        bank,
    ))
}

fn selection_track(mode: u8, count: u32) -> Result<TestHircObject, Box<dyn Error>> {
    const PLAIN_NODE_HEADER: usize = 31;
    const EMPTY_AUTOMATION_WORDS: usize = 4;
    let fixture = music_track_with_empty_automation(10, &[FIRST_SOURCE, SECOND_SOURCE])?;
    let mut body = hirc_payload(fixture)?;
    let kind = body
        .len()
        .checked_sub(ID_BYTES + size_of::<u8>())
        .ok_or("the kind byte precedes the look-ahead word")?;
    let count_end = kind
        .checked_sub(PLAIN_NODE_HEADER + EMPTY_AUTOMATION_WORDS * ID_BYTES)
        .ok_or("the count precedes the automation and node records")?;
    let count_bytes = body
        .get_mut(count_end - ID_BYTES..count_end)
        .ok_or("the fixture subtrack count word")?;
    assert_eq!(
        count_bytes, [0; ID_BYTES],
        "the fixture starts with an explicit zero count"
    );
    count_bytes.copy_from_slice(&count.to_le_bytes());
    let kind_byte = body.get_mut(kind).ok_or("the normal fixture kind")?;
    assert_eq!(*kind_byte, 0, "the initial kind is Normal");
    *kind_byte = mode;
    for (authored, source) in [(1, FIRST_SOURCE), (2, SECOND_SOURCE)] {
        let row = clip_words(&mut body, authored, source, 0, [0.0, 0.0, 0.0, 10.0])?;
        let index = if count == TWO_SUBTRACKS {
            authored - 1
        } else {
            0
        };
        body.get_mut(row..row + ID_BYTES)
            .ok_or("the clip starts with its subtrack index")?
            .copy_from_slice(&index.to_le_bytes());
    }
    Ok(TestHircObject::raw(11, 10, body))
}
