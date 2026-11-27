// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Soundbank;
use p4kx_testkit::{TestBank, TestHircObject};
use serde_json::{Value, json};

use super::{Error, TestResult, json as list_json, write_bank};

const PLAYLIST: u32 = 11;
const TRIGGER: u32 = 0x5100_0001;
const SEGMENT: u32 = 0x5100_0002;
const WORD: usize = size_of::<u32>();

#[test]
fn music_playlist_stinger_source_retains_order_and_checked_malformed_prefix() -> TestResult {
    let report = view(154, playlist_with_stingers()?)?;
    let source = report
        .pointer("/objects/0/source_fields")
        .ok_or("MusicPlaylist source fields")?;
    assert_eq!(source.get("kind"), Some(&json!("music-playlist")), "kind");
    assert_eq!(
        source.pointer("/layout/value"),
        Some(&json!("malformed")),
        "/layout/value"
    );
    assert_eq!(
        source.pointer("/stinger_count/value"),
        Some(&json!(2)),
        "/stinger_count/value"
    );
    let rows = source["stingers"]
        .as_array()
        .ok_or("original stinger rows")?;
    assert_eq!(rows.len(), 2, "original stinger order and count");
    let [complete, partial] = rows.as_slice() else {
        return Err("complete row and checked tail expected".into());
    };
    assert_complete(complete);
    assert_eq!(
        partial.pointer("/ordinal/value"),
        Some(&json!(1)),
        "/ordinal/value"
    );
    assert_eq!(
        partial.pointer("/trigger_id/value"),
        Some(&json!(TRIGGER)),
        "/trigger_id/value"
    );
    assert_eq!(
        partial.pointer("/sync_play_at/value"),
        Some(&json!(19)),
        "/sync_play_at/value"
    );
    assert_eq!(
        partial.pointer("/complete/value"),
        Some(&json!(false)),
        "/complete/value"
    );
    assert!(partial.get("cue_filter_hash").is_none(), "cue_filter_hash");
    assert!(
        partial.get("dont_repeat_time").is_none(),
        "dont_repeat_time"
    );
    Ok(())
}

#[test]
fn old_music_playlist_layout_exposes_unsupported_source() -> TestResult {
    let report = view(153, TestHircObject::music_playlist(PLAYLIST, &[SEGMENT]))?;
    let source = report
        .pointer("/objects/0/source_fields")
        .ok_or("MusicPlaylist source fields")?;
    assert_eq!(
        source.pointer("/layout/value"),
        Some(&json!("unsupported")),
        "/layout/value"
    );
    assert_eq!(
        source.pointer("/version/value"),
        Some(&json!(153)),
        "/version/value"
    );
    assert!(source.get("stinger_count").is_none(), "stinger_count");
    assert_eq!(source.get("stingers"), Some(&json!([])), "stingers");
    Ok(())
}

#[test]
fn segment_and_switch_share_the_original_stinger_contract() -> TestResult {
    for (kind, object) in [
        (
            "music-segment",
            TestHircObject::music_segment_with_stingers(PLAYLIST, &[9], &[(TRIGGER, SEGMENT)]),
        ),
        (
            "music-switch",
            TestHircObject::music_switch(PLAYLIST, &[9], 7, &[9]),
        ),
    ] {
        let report = view(154, object)?;
        let source = report
            .pointer("/objects/0/source_fields")
            .ok_or("music node source fields")?;
        assert_eq!(source.get("kind"), Some(&json!(kind)), "owner kind");
        for (field, value) in [
            ("/layout/value", json!("decoded")),
            ("/stinger_count/value", json!(1)),
            ("/stingers/0/ordinal/value", json!(0)),
            ("/stingers/0/complete/value", json!(true)),
            ("/stingers/0/trigger_id/value", json!(TRIGGER)),
            ("/stingers/0/segment_id/value", json!(SEGMENT)),
            ("/stingers/0/cue_filter_hash/value", json!(0)),
        ] {
            assert_eq!(source.pointer(field), Some(&value), "{kind} field {field}");
        }
    }
    Ok(())
}

fn assert_complete(row: &Value) {
    for (path, value) in [
        ("/ordinal/value", json!(0)),
        ("/complete/value", json!(true)),
        ("/trigger_id/value", json!(TRIGGER)),
        ("/segment_id/value", json!(SEGMENT)),
        ("/sync_play_at/value", json!(u32::MAX)),
        ("/cue_filter_hash/value", json!(31)),
        ("/dont_repeat_time/bits/value", json!(0xFFFF_FFF9_u32)),
        ("/dont_repeat_time/signed_value/value", json!(-7)),
        ("/segment_look_ahead/value", json!(5)),
    ] {
        assert_eq!(
            row.pointer(path),
            Some(&value),
            "original stinger word {path}"
        );
    }
    assert_eq!(
        row.pointer("/dont_repeat_time/bits/containsEncryptedSource"),
        Some(&Value::Null),
        "standalone BNK lineage",
    );
}

fn playlist_with_stingers() -> Result<TestHircObject, Box<dyn Error>> {
    let bytes = TestBank::new(21)
        .version(154)
        .object(TestHircObject::music_playlist(PLAYLIST, &[SEGMENT]))
        .bytes();
    let bank = Soundbank::parse(&bytes)?;
    let object = bank.objects().first().ok_or("synthetic playlist object")?;
    let body = bank
        .object_body(object)
        .get(WORD..)
        .ok_or("playlist body")?;
    let marker = [TRIGGER.to_le_bytes(), SEGMENT.to_le_bytes()];
    let marker = marker.as_flattened();
    let position = body
        .windows(marker.len())
        .position(|bytes| bytes == marker)
        .ok_or("original fixture stinger")?;
    let start = position.checked_sub(WORD).ok_or("stinger count start")?;
    let mut prefix = body.get(..start).ok_or("playlist prefix")?.to_vec();
    let complete = [2, TRIGGER, SEGMENT, u32::MAX, 31, 0xFFFF_FFF9, 5];
    prefix.extend(complete.into_iter().flat_map(u32::to_le_bytes));
    prefix.extend(
        [TRIGGER, SEGMENT, 19]
            .into_iter()
            .flat_map(u32::to_le_bytes),
    );
    prefix.extend([0xAB, 0xCD]);
    Ok(TestHircObject::raw(13, PLAYLIST, prefix))
}

fn view(version: u32, object: TestHircObject) -> Result<Value, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let bank = TestBank::new(21).version(version).object(object);
    let path = write_bank(directory.path(), &bank)?;
    list_json(&["--id", &PLAYLIST.to_string()], &path)
}
