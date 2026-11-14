// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestHircObject};
use serde_json::{Value, json};

use super::{TestResult, json as list_json, plain_node_prefix, write_bank};

const OBJECT_ID: u32 = 0x5AC2_6400;
const CHILD_A: u32 = 0xA042_6401;
const CHILD_B: u32 = 0xB042_6402;
const GROUP_ID: u32 = 0x9422_6430;

#[test]
fn switch_listing_preserves_original_defaults_repeated_members_and_signed_fades() -> TestResult {
    let directory = tempfile::tempdir()?;
    let mut body = header()?;
    words(&mut body, [2, 0, 3, CHILD_B, CHILD_A, CHILD_B, 0, 0, 2]);
    configuration(&mut body, CHILD_A, 0xE7, i32::MIN, -17);
    configuration(&mut body, CHILD_A, 0x03, 0, i32::MAX);
    let path = write_bank(directory.path(), &bank(154, body))?;
    let result = list_json(&["--id", &OBJECT_ID.to_string()], &path)?;
    let source = result
        .pointer("/objects/0/source_fields")
        .ok_or("Switch source fields")?;
    assert_eq!(
        source["kind"], "switch-container",
        "typed source field kind"
    );
    assert_eq!(
        source["layout"]["value"], "decoded",
        "original declaration fits"
    );
    let fields = &source["fields"];
    assert_eq!(
        fields["group_type"]["value"], 0xFE,
        "unknown original group code"
    );
    assert!(
        fields.get("group_kind").is_none(),
        "unknown type has no classification"
    );
    assert_eq!(fields["group_id"]["value"], GROUP_ID, "original group ID");
    assert_eq!(
        fields["default_value_id"]["value"], 0,
        "explicit zero default"
    );
    assert_eq!(
        fields["continuous_validation"]["value"], 0xB0,
        "original flag byte"
    );
    assert_eq!(
        fields["groups"]["declared_count"]["value"], 2,
        "original group count"
    );
    let rows = source["rows"].as_array().ok_or("source rows")?;
    memberships(rows)?;
    fades(
        rows.get(5).ok_or("first configuration")?,
        rows.get(6).ok_or("second configuration")?,
    );
    Ok(())
}

fn fades(first: &Value, second: &Value) {
    assert_eq!(
        first.pointer("/fade_out/signed_value/value"),
        Some(&json!(i32::MIN)),
        "signed fade-out"
    );
    assert_eq!(
        first.pointer("/fade_in/bits/value"),
        Some(&json!(u32::from_le_bytes((-17_i32).to_le_bytes()))),
        "exact original fade-in bits"
    );
    assert_eq!(
        second.pointer("/fade_out/signed_value/value"),
        Some(&json!(0)),
        "explicit zero fade"
    );
    assert_eq!(
        second.pointer("/flags/value"),
        Some(&json!(3)),
        "second occurrence with the same child ID"
    );
}

fn memberships(rows: &[Value]) -> TestResult {
    let kinds: Vec<_> = rows
        .iter()
        .filter_map(|row| row.get("kind").and_then(Value::as_str))
        .collect();
    assert_eq!(
        kinds,
        [
            "group",
            "member",
            "member",
            "member",
            "group",
            "configuration",
            "configuration"
        ],
        "original physical row order"
    );
    let ids: Vec<_> = rows
        .iter()
        .filter(|row| row.get("kind") == Some(&json!("member")))
        .filter_map(|row| row.pointer("/child_id/value").and_then(Value::as_u64))
        .collect();
    assert_eq!(
        ids,
        [CHILD_B, CHILD_A, CHILD_B].map(u64::from),
        "original repeats"
    );
    let group = rows.get(4).ok_or("empty repeated group")?;
    assert_eq!(
        group.pointer("/value_id/value"),
        Some(&json!(0)),
        "the duplicate default group remains distinct"
    );
    assert_eq!(
        group.pointer("/member_count/value"),
        Some(&json!(0)),
        "explicit empty group"
    );
    Ok(())
}

#[test]
fn switch_listing_distinguishes_partial_missing_and_unsupported_source_coverage() -> TestResult {
    let directory = tempfile::tempdir()?;
    let mut partial = header()?;
    words(&mut partial, [1, 0, u32::MAX, CHILD_B]);
    partial.extend([0xEF, 0xCC]);
    for (version, layout, body) in [
        (154, "malformed", partial),
        (154, "unavailable", vec![0]),
        (153, "unsupported", header()?),
    ] {
        let path = write_bank(directory.path(), &bank(version, body))?;
        let result = list_json(&["--id", &OBJECT_ID.to_string()], &path)?;
        let source = result
            .pointer("/objects/0/source_fields")
            .ok_or("Switch coverage")?;
        assert_eq!(
            source["layout"]["value"], layout,
            "source coverage disposition"
        );
        if layout == "malformed" {
            assert_eq!(
                source["rows"][0]["member_count"]["value"],
                u32::MAX,
                "original impossible count"
            );
            assert_eq!(
                source["rows"][1]["child_id"]["value"], CHILD_B,
                "checked member prefix"
            );
            assert!(
                source["fields"].get("configurations").is_none(),
                "configuration table is beyond the prefix"
            );
        } else {
            assert_eq!(
                source["rows"],
                json!([]),
                "no checked source rows are available"
            );
        }
    }
    Ok(())
}

fn bank(version: u32, body: Vec<u8>) -> TestBank {
    TestBank::new(17)
        .version(version)
        .object(TestHircObject::event(1, &[]))
        .object(TestHircObject::raw(6, OBJECT_ID, body))
}

fn header() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut body = plain_node_prefix()?;
    body.push(0xFE);
    words(&mut body, [GROUP_ID, 0]);
    body.push(0xB0);
    words(&mut body, [2, CHILD_A, CHILD_B]);
    Ok(body)
}

fn words<const N: usize>(body: &mut Vec<u8>, words: [u32; N]) {
    body.extend(words.into_iter().flat_map(u32::to_le_bytes));
}

fn configuration(body: &mut Vec<u8>, child: u32, flags: u8, fade_out: i32, fade_in: i32) {
    body.extend(child.to_le_bytes());
    body.push(flags);
    body.extend(fade_out.to_le_bytes());
    body.extend(fade_in.to_le_bytes());
}
