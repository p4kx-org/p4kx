// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestHircObject};
use serde_json::{Value, json};

use super::{TestResult, json as list_json, write_bank};

const OBJECT_ID: u32 = 0xDB24_5410;
const SOURCE_ID: u32 = 0xA240_54C1;
const STATE_ID: u32 = 0xB040_54D3;

#[test]
fn music_track_source_fields_keep_original_clip_and_switch_order() -> TestResult {
    let object = TestHircObject::music_state_track(
        OBJECT_ID,
        &[SOURCE_ID, SOURCE_ID],
        0,
        &[STATE_ID, STATE_ID],
    );
    let report = view(154, object)?;
    let source = report
        .pointer("/objects/0/source_fields")
        .ok_or("original MusicTrack fields")?;
    assert_eq!(
        source.get("kind"),
        Some(&json!("music-track")),
        "typed object"
    );
    assert_eq!(
        source.pointer("/layout/value"),
        Some(&json!("decoded")),
        "known layout fits"
    );
    check_clip_rows(source)
}

fn check_clip_rows(source: &Value) -> TestResult {
    let rows = source
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("MusicTrack rows")?;
    let kinds: Vec<_> = rows
        .iter()
        .filter_map(|row| row.get("kind").and_then(Value::as_str))
        .collect();
    assert_eq!(
        kinds,
        [
            "media-source",
            "media-source",
            "clip",
            "clip",
            "automation",
            "automation-point",
            "automation-point",
            "switch-member",
            "switch-member"
        ],
        "original physical order"
    );
    for (path, value) in [
        ("/fields/source_count/value", 2),
        ("/fields/clip_count/value", 2),
        ("/fields/subtrack_count/value", 0),
        ("/fields/switch/group_id/value", 0),
        ("/fields/switch/member_count/value", 2),
        ("/fields/look_ahead/signed_value/value", 0),
        ("/rows/2/track_id/value", 1),
        ("/rows/3/track_id/value", 2),
        ("/rows/2/source_id/value", SOURCE_ID),
        ("/rows/3/source_id/value", SOURCE_ID),
        ("/rows/7/value_id/value", STATE_ID),
        ("/rows/8/value_id/value", STATE_ID),
    ] {
        assert_eq!(source.pointer(path), Some(&json!(value)), "{path}");
    }
    assert_eq!(
        source.pointer("/rows/2/begin_trim/bits/value"),
        Some(&json!("3fe0000000000000")),
        "exact binary64 trim word"
    );
    assert_eq!(
        source.pointer("/rows/2/begin_trim/finite_value/value"),
        Some(&json!(0.5)),
        "finite numeric view"
    );
    assert_eq!(
        source.pointer("/rows/2/begin_trim/bits/containsEncryptedSource"),
        Some(&Value::Null),
        "standalone P4K lineage is unknown"
    );
    Ok(())
}

#[test]
fn music_track_source_fields_keep_checked_automation_words_in_malformed_track() -> TestResult {
    let mut body = 0_u32.to_le_bytes().to_vec();
    body.push(0xE0);
    body.extend(
        [0_u32, 1, 0, u32::MAX, u32::MAX, 0x7FC1_5410, 0x8000_0000]
            .into_iter()
            .flat_map(u32::to_le_bytes),
    );
    body.extend([0xA1, 0xB2]);
    let report = view(154, TestHircObject::raw(11, OBJECT_ID, body.clone()))?;
    let source = report
        .pointer("/objects/0/source_fields")
        .ok_or("original MusicTrack fields")?;
    assert_partial_automation(source);
    let previous = view(153, TestHircObject::raw(11, OBJECT_ID, body))?;
    assert_eq!(
        previous.pointer("/objects/0/source_fields/layout/value"),
        Some(&json!("unsupported")),
        "older source layout"
    );
    assert_eq!(
        previous.pointer("/objects/0/source_fields/rows"),
        Some(&json!([])),
        "no claimed source rows for an unsupported layout"
    );
    Ok(())
}

fn assert_partial_automation(source: &Value) {
    assert_eq!(
        source.pointer("/layout/value"),
        Some(&json!("malformed")),
        "object-level truncation"
    );
    assert_eq!(
        source.pointer("/fields/flags/value"),
        Some(&json!(0xE0)),
        "original flags before truncation"
    );
    assert_eq!(
        source.pointer("/fields/automation/complete/value"),
        Some(&json!(false)),
        "original table incomplete"
    );
    assert_eq!(
        source.pointer("/rows/0/clip_index/value"),
        Some(&json!(0)),
        "original clip selector"
    );
    assert_eq!(
        source.pointer("/rows/0/points/declared_count/value"),
        Some(&json!(u32::MAX)),
        "full original impossible count"
    );
    assert_eq!(
        source.pointer("/rows/1/from_bits/value"),
        Some(&json!(0x7FC1_5410_u32)),
        "original nonfinite word"
    );
    assert_eq!(
        source.pointer("/rows/1/to_bits/value"),
        Some(&json!(0x8000_0000_u32)),
        "original negative zero"
    );
    assert!(
        source.pointer("/rows/1/interpolation").is_none(),
        "partial word is not fabricated"
    );
}

fn view(version: u32, object: TestHircObject) -> Result<Value, Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let bank = TestBank::new(21).version(version).object(object);
    let path = write_bank(directory.path(), &bank)?;
    list_json(&["--id", &OBJECT_ID.to_string()], &path)
}
