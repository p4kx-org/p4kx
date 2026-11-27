// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Soundbank;
use p4kx_testkit::{TestBank, TestHircObject};
use serde_json::{Value, json};

use super::{Error, TestResult, json as list_json, write_bank};

const OWNER: u32 = 11;
const CHILD: u32 = 13;
const PLAYLIST_TYPE: u8 = 13;
const WORD: usize = size_of::<u32>();
const STINGER_WORDS: [u32; 6] = [0x5100_0001, 0x5100_0002, 0, 0, 0, 0];
const DEFAULT_ID_LIST_WORDS: usize = 5;
const SOURCE_RULE_BYTES: usize = 21;
const DESTINATION_RULE_BYTES: usize = 26;
const RULE_BYTES: usize =
    DEFAULT_ID_LIST_WORDS * WORD + SOURCE_RULE_BYTES + DESTINATION_RULE_BYTES + 1;
const ALLOCATED_OBJECT_BYTES: usize = 30;
const TABLE_BYTES: usize = WORD + 2 * RULE_BYTES + ALLOCATED_OBJECT_BYTES;

#[test]
fn playlist_and_switch_expose_the_same_original_rule_and_id_order() -> TestResult {
    for (kind, owner) in [
        (
            "music-playlist",
            TestHircObject::music_playlist(OWNER, &[CHILD]),
        ),
        (
            "music-switch",
            TestHircObject::music_switch(OWNER, &[CHILD], 17, &[CHILD]),
        ),
    ] {
        let report = view(154, owner)?;
        let source = report
            .pointer("/objects/0/source_fields")
            .ok_or("owner source fields")?;
        assert_eq!(source.get("kind"), Some(&json!(kind)), "original owner");
        assert_eq!(
            source.pointer("/stingers/0/trigger_id/value"),
            Some(&json!(STINGER_WORDS[0])),
            "the original stinger is present alongside transition rules"
        );
        let table = transitions(&report)?;
        for (path, value) in [
            ("/layout/value", json!("decoded")),
            ("/declared_count/value", json!(2)),
            ("/complete_rules/value", json!(2)),
            ("/rows/0/kind", json!("rule")),
            ("/rows/0/source_ids/declared_count/value", json!(1)),
            ("/rows/0/destination_ids/declared_count/value", json!(2)),
            ("/rows/0/allocation_flag/value", json!(0)),
            ("/rows/1/side/value", json!("source")),
            ("/rows/1/id/value", json!(0x5200_0001_u32)),
            ("/rows/2/side/value", json!("destination")),
            ("/rows/2/id/value", json!(0x5200_0002_u32)),
            ("/rows/3/ordinal/value", json!(1)),
            ("/rows/3/id/value", json!(0x5200_0003_u32)),
            ("/rows/4/ordinal/value", json!(1)),
            ("/rows/4/allocation_flag/value", json!(1)),
            ("/rows/4/transition_object/segment_id/value", json!(0)),
            ("/rows/7/rule_ordinal/value", json!(1)),
        ] {
            assert_eq!(
                table.pointer(path),
                Some(&value),
                "{kind} original field {path}"
            );
        }
        assert_eq!(
            table["rows"].as_array().map(Vec::len),
            Some(8),
            "two separate rule declarations and six ID occurrences"
        );
    }
    Ok(())
}

#[test]
fn a_partial_final_rule_keeps_its_readable_id_and_a_later_failure_keeps_the_table_complete()
-> TestResult {
    let fixture = PlaylistFixture::read()?;
    let partial_end = WORD + RULE_BYTES + 2 * WORD + 2;
    let partial = fixture.prefix(partial_end)?;
    let report = view(154, partial)?;
    let table = transitions(&report)?;
    for (path, value) in [
        ("/layout/value", json!("malformed")),
        ("/declared_count/value", json!(2)),
        ("/complete_rules/value", json!(1)),
        ("/rows/4/complete/value", json!(false)),
        ("/rows/5/id/value", json!(0x5200_0001_u32)),
    ] {
        assert_eq!(
            table.pointer(path),
            Some(&value),
            "the checked partial table keeps {path}"
        );
    }
    assert_eq!(
        table["rows"].as_array().map(Vec::len),
        Some(6),
        "the unreadable count and absent destination IDs add no rows"
    );

    let report = view(154, fixture.prefix(TABLE_BYTES)?)?;
    assert_eq!(
        report.pointer("/objects/0/source_fields/layout/value"),
        Some(&json!("malformed")),
        "the playlist suffix is missing"
    );
    let table = transitions(&report)?;
    assert_eq!(
        table.pointer("/layout/value"),
        Some(&json!("decoded")),
        "the checked transition table preceded the failed suffix"
    );
    assert_eq!(
        table.pointer("/complete_rules/value"),
        Some(&json!(2)),
        "both complete original rules remain"
    );
    Ok(())
}

#[test]
fn empty_unavailable_and_unsupported_tables_have_distinct_states() -> TestResult {
    let fixture = PlaylistFixture::read()?;
    let mut empty = fixture
        .body
        .get(..fixture.table_start)
        .ok_or("music node prefix")?
        .to_vec();
    empty.extend(0_u32.to_le_bytes());
    empty.extend_from_slice(
        fixture
            .body
            .get(fixture.table_start + TABLE_BYTES..)
            .ok_or("playlist suffix")?,
    );
    let report = view(154, TestHircObject::raw(PLAYLIST_TYPE, OWNER, empty))?;
    let table = transitions(&report)?;
    assert_eq!(
        table.pointer("/layout/value"),
        Some(&json!("decoded")),
        "a stored zero is a decoded table"
    );
    assert_eq!(
        table.pointer("/declared_count/value"),
        Some(&json!(0)),
        "explicit empty authored count"
    );
    assert_eq!(
        table.get("rows"),
        Some(&json!([])),
        "no rows in the empty table"
    );

    let report = view(154, fixture.prefix(2)?)?;
    let table = transitions(&report)?;
    assert_eq!(
        table.pointer("/layout/value"),
        Some(&json!("unavailable")),
        "the original count is incomplete"
    );
    assert!(
        table.get("declared_count").is_none(),
        "an unreadable count is not fabricated"
    );

    let report = view(153, TestHircObject::music_playlist(OWNER, &[CHILD]))?;
    let table = transitions(&report)?;
    assert_eq!(
        table.pointer("/layout/value"),
        Some(&json!("unsupported")),
        "no v153 layout claim"
    );
    assert_eq!(
        table.get("rows"),
        Some(&json!([])),
        "no invented old-version rules"
    );
    Ok(())
}

#[test]
fn signed_fades_and_unknown_flags_stay_with_the_original_rule_and_side() -> TestResult {
    let mut fixture = PlaylistFixture::read()?;
    let rule = fixture.table_start + WORD;
    let source = rule + DEFAULT_ID_LIST_WORDS * WORD;
    let destination = source + SOURCE_RULE_BYTES;
    let allocated_segment = rule + 2 * RULE_BYTES;
    fixture.set_word(source, (-400_i32).to_le_bytes())?;
    fixture.set_word(source + 3 * WORD, u32::MAX.to_le_bytes())?;
    fixture.set_word(destination, 900_u32.to_le_bytes())?;
    fixture.set_word(destination + 2 * WORD, i32::MIN.to_le_bytes())?;
    fixture.set_word(allocated_segment, CHILD.to_le_bytes())?;
    let allocation = fixture
        .body
        .get_mut(allocated_segment - 1)
        .ok_or("second allocation flag")?;
    *allocation = 0xA0;

    let report = view(154, TestHircObject::raw(PLAYLIST_TYPE, OWNER, fixture.body))?;
    let table = transitions(&report)?;
    for (path, value) in [
        (
            "/rows/0/source/fade/time/bits/value",
            json!(u32::from_le_bytes((-400_i32).to_le_bytes())),
        ),
        ("/rows/0/source/fade/time/signed_value/value", json!(-400)),
        ("/rows/0/source/sync_type/value", json!(u32::MAX)),
        (
            "/rows/0/destination/fade/time/signed_value/value",
            json!(900),
        ),
        (
            "/rows/0/destination/fade/offset/signed_value/value",
            json!(i32::MIN),
        ),
        ("/rows/4/allocation_flag/value", json!(0xA0)),
        ("/rows/4/transition_object/segment_id/value", json!(CHILD)),
    ] {
        assert_eq!(
            table.pointer(path),
            Some(&value),
            "stored rule field {path}"
        );
    }
    Ok(())
}

struct PlaylistFixture {
    body: Vec<u8>,
    table_start: usize,
}

impl PlaylistFixture {
    fn read() -> Result<Self, Box<dyn Error>> {
        let original = TestBank::new(21)
            .version(154)
            .object(TestHircObject::music_playlist(OWNER, &[CHILD]))
            .bytes();
        let bank = Soundbank::parse(&original)?;
        let object = bank.objects().first().ok_or("synthetic playlist")?;
        let body = bank
            .object_body(object)
            .get(WORD..)
            .ok_or("original body")?;
        let stinger = STINGER_WORDS.map(u32::to_le_bytes);
        let stinger = stinger.as_flattened();
        let table_start = body
            .windows(stinger.len())
            .position(|bytes| bytes == stinger)
            .ok_or("original stinger words")?
            + stinger.len();
        assert_eq!(
            body.get(table_start..table_start + WORD),
            Some(2_u32.to_le_bytes().as_slice()),
            "fixture transition count"
        );
        Ok(Self {
            body: body.to_vec(),
            table_start,
        })
    }

    fn set_word(&mut self, offset: usize, bytes: [u8; WORD]) -> TestResult {
        self.body
            .get_mut(offset..offset + WORD)
            .ok_or("authored transition word")?
            .copy_from_slice(&bytes);
        Ok(())
    }

    fn prefix(&self, table_bytes: usize) -> Result<TestHircObject, Box<dyn Error>> {
        let prefix = self
            .body
            .get(..self.table_start + table_bytes)
            .ok_or("original partial table extent")?;
        Ok(TestHircObject::raw(PLAYLIST_TYPE, OWNER, prefix.to_vec()))
    }
}

fn transitions(report: &Value) -> Result<&Value, Box<dyn Error>> {
    report
        .pointer("/objects/0/source_fields/transition_rules")
        .ok_or_else(|| "original transition table".into())
}

fn view(version: u32, object: TestHircObject) -> Result<Value, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let bank = TestBank::new(21).version(version).object(object);
    let path = write_bank(directory.path(), &bank)?;
    list_json(&["--id", &OWNER.to_string()], &path)
}
