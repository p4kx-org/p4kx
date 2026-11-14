// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestHircObject};
use serde_json::{Value, json};

use super::{TestResult, json as list_json, plain_node_prefix, write_bank};

const OBJECT_ID: u32 = 0xFA12_0500;
const CHILD_A: u32 = 0xA122_0501;
const CHILD_B: u32 = 0xB122_0502;
const PLAYLIST: [(u32, i32); 3] = [(CHILD_B, i32::MIN), (CHILD_A, -2), (CHILD_B, 0)];

#[test]
fn source_listing_keeps_random_sequence_modes_timing_words_and_signed_order() -> TestResult {
    let directory = tempfile::tempdir()?;
    let mut body = original_header()?;
    body.extend(3_u16.to_le_bytes());
    for (id, weight) in PLAYLIST {
        body.extend(id.to_le_bytes());
        body.extend(weight.to_le_bytes());
    }
    let path = write_bank(directory.path(), &bank(154, body))?;
    let result = list_json(&["--id", &OBJECT_ID.to_string()], &path)?;
    let source = result
        .pointer("/objects/0/source_fields")
        .ok_or("Random/Sequence original fields")?;
    assert_eq!(
        result.pointer("/objects/0/detail/container_kind"),
        Some(&json!("sequence")),
        "eager mode"
    );
    assert_eq!(
        source["kind"], "random-sequence-container",
        "source field kind"
    );
    assert_eq!(source["layout"]["value"], "decoded", "physical fields fit");
    let fields = &source["fields"];
    assert_eq!(fields["loop_count"]["value"], 0, "explicit zero loop count");
    assert_eq!(fields["mode"]["value"], 1, "authored sequence mode");
    assert_eq!(fields["flags"]["value"], 0xE7, "full flags byte");
    assert_eq!(
        fields["transition_mode"]["value"], 0xFE,
        "unknown transition code"
    );
    assert_eq!(
        fields["transition_time"]["bits"]["value"], 0x7FC1_2345_u32,
        "raw NaN bits"
    );
    assert!(
        fields["transition_time"].get("finite_value").is_none(),
        "NaN stays raw"
    );
    assert_eq!(
        fields["transition_minimum"]["finite_value"]["value"], 2.5,
        "original finite timing"
    );
    assert_eq!(
        fields["playlist"]["declared_count"]["value"], 3,
        "original count"
    );
    playlist_evidence(source)?;
    Ok(())
}

fn playlist_evidence(source: &Value) -> TestResult {
    let rows = source
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("original playlist rows")?;
    assert_eq!(rows.len(), PLAYLIST.len(), "all original rows");
    for (row, (id, weight)) in rows.iter().zip(PLAYLIST) {
        assert_eq!(
            row.pointer("/play_id/value"),
            Some(&json!(id)),
            "original order and duplicates"
        );
        assert_eq!(
            row.pointer("/weight/value"),
            Some(&json!(weight)),
            "signed original weight"
        );
        assert_eq!(
            row.pointer("/weight_bits/value"),
            Some(&json!(u32::from_le_bytes(weight.to_le_bytes()))),
            "exact original bits"
        );
        assert_eq!(
            row.pointer("/weight/sourceSetComplete"),
            Some(&json!(true)),
            "direct source receipt"
        );
    }
    Ok(())
}

#[test]
fn malformed_playlist_keeps_the_play_id_and_unknown_versions_keep_raw_coverage() -> TestResult {
    let directory = tempfile::tempdir()?;
    let mut body = original_header()?;
    body.extend(2_u16.to_le_bytes());
    body.extend(CHILD_A.to_le_bytes());
    body.extend(0_u32.to_le_bytes());
    body.extend(CHILD_B.to_le_bytes());
    body.extend([0xEF, 0xCC]);
    for (version, status) in [(154, "malformed"), (153, "unsupported")] {
        let path = write_bank(directory.path(), &bank(version, body.clone()))?;
        let result = list_json(&["--id", &OBJECT_ID.to_string()], &path)?;
        let source = result
            .pointer("/objects/0/source_fields")
            .ok_or("source coverage")?;
        assert_eq!(
            source["layout"]["value"], status,
            "version-qualified decision coverage"
        );
        if version == 154 {
            assert_eq!(
                source["rows"][1]["play_id"]["value"], CHILD_B,
                "checked final ID"
            );
            assert!(
                source["rows"][1].get("weight_bits").is_none(),
                "partial weight is absent"
            );
            assert_eq!(
                source["fields"]["playlist"]["complete"]["value"], false,
                "original table incomplete"
            );
        } else {
            assert_eq!(
                source["rows"],
                json!([]),
                "unsupported version has no typed rows"
            );
        }
    }
    Ok(())
}

fn bank(version: u32, body: Vec<u8>) -> TestBank {
    TestBank::new(9)
        .version(version)
        .object(TestHircObject::event(1, &[]))
        .object(TestHircObject::raw(5, OBJECT_ID, body))
}

fn original_header() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = plain_node_prefix()?;
    bytes.extend([0_u16, 3, u16::MAX].into_iter().flat_map(u16::to_le_bytes));
    bytes.extend(
        [0x7FC1_2345_u32, 2.5_f32.to_bits(), 0x7F80_0000]
            .into_iter()
            .flat_map(u32::to_le_bytes),
    );
    bytes.extend(17_u16.to_le_bytes());
    bytes.extend([0xFE, 0xFD, 1, 0xE7]);
    bytes.extend([2, CHILD_A, CHILD_B].into_iter().flat_map(u32::to_le_bytes));
    Ok(bytes)
}
