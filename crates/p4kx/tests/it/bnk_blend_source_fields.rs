// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestHircObject};
use serde_json::{Value, json};

use super::{TestResult, json as list_json, plain_node_prefix, write_bank};

const OBJECT_ID: u32 = 0x9B04_1122;
const CHILD_A: u32 = 0xA137_5511;
const CHILD_B: u32 = 0xB157_6612;
const LAYER: u32 = 0x9AA0_3300;
const CROSSFADE: u32 = 0xCC12_2030;
const CURVE: u32 = 0xE152_4020;
const NONFINITE: u32 = 0x7FC1_1234;
const NEGATIVE_ZERO: u32 = 0x8000_0000;

#[test]
fn blend_source_fields_keep_repeated_layers_and_original_graph_words() -> TestResult {
    let mut body = children()?;
    words(&mut body, [2, LAYER]);
    initial_curve(&mut body);
    crossfade(&mut body, CROSSFADE, 0xFE, 2);
    words(
        &mut body,
        [
            CHILD_B,
            1,
            NONFINITE,
            NEGATIVE_ZERO,
            u32::MAX,
            CHILD_B,
            0,
            LAYER,
        ],
    );
    body.extend(0_u16.to_le_bytes());
    crossfade(&mut body, 0, 0, 0);
    body.push(0xE0);
    let bank = bank(154, body);
    let report = view(&bank)?;
    let source = report
        .pointer("/objects/0/source_fields")
        .ok_or("Blend source fields")?;
    assert_eq!(source.get("kind"), Some(&json!("blend-container")));
    assert_eq!(source.pointer("/layout/value"), Some(&json!("decoded")));
    let rows = source
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("original Blend rows")?;
    let kinds: Vec<_> = rows
        .iter()
        .filter_map(|row| row.get("kind").and_then(Value::as_str))
        .collect();
    assert_eq!(
        kinds,
        [
            "child",
            "child",
            "layer",
            "rtpc-curve",
            "rtpc-point",
            "association",
            "crossfade-point",
            "association",
            "layer"
        ]
    );
    checked_fields(source);
    let input = source
        .pointer("/rows/6/from_bits")
        .ok_or("original crossfade input")?;
    exact_word(source, input, &bank.bytes())
}

fn checked_fields(source: &Value) {
    for (path, value) in [
        ("/fields/layers/declared_count/value", 2),
        ("/fields/continuous_validation/value", 0xE0),
        ("/rows/0/child_id/value", CHILD_A),
        ("/rows/2/layer_id/value", LAYER),
        ("/rows/2/crossfade_rtpc_type/value", 0xFE),
        ("/rows/3/curve_id/value", CURVE),
        ("/rows/4/interpolation/value", 6),
        ("/rows/5/child_id/value", CHILD_B),
        ("/rows/6/from_bits/value", NONFINITE),
        ("/rows/6/to_bits/value", NEGATIVE_ZERO),
        ("/rows/6/interpolation/value", u32::MAX),
        ("/rows/7/points/declared_count/value", 0),
        ("/rows/8/layer_id/value", LAYER),
        ("/rows/8/layer_ordinal/value", 1),
        ("/rows/8/crossfade_rtpc_id/value", 0),
    ] {
        assert_eq!(source.pointer(path), Some(&json!(value)), "{path}");
    }
}

#[test]
fn blend_source_fields_keep_partial_words_and_explicit_layout_coverage() -> TestResult {
    let mut body = children()?;
    words(&mut body, [1, LAYER]);
    initial_curve(&mut body);
    crossfade(&mut body, CROSSFADE, 0xFE, 1);
    words(&mut body, [CHILD_B, u32::MAX, NONFINITE, NEGATIVE_ZERO]);
    body.extend([0xCD, 0xE5]);
    let report = view(&bank(154, body.clone()))?;
    let source = report
        .pointer("/objects/0/source_fields")
        .ok_or("Blend source fields")?;
    assert_eq!(source.pointer("/layout/value"), Some(&json!("malformed")));
    assert_eq!(
        source.pointer("/fields/layers/complete/value"),
        Some(&json!(false))
    );
    assert!(source.pointer("/fields/continuous_validation").is_none());
    assert_eq!(
        source.pointer("/rows/2/initial_rtpc/complete/value"),
        Some(&json!(true))
    );
    assert_eq!(
        source.pointer("/rows/4/interpolation/value"),
        Some(&json!(6))
    );
    assert_eq!(
        source.pointer("/rows/5/points/declared_count/value"),
        Some(&json!(u32::MAX))
    );
    assert_eq!(
        source.pointer("/rows/6/to_bits/value"),
        Some(&json!(NEGATIVE_ZERO))
    );
    assert!(source.pointer("/rows/6/interpolation").is_none());
    for (version, bytes, layout) in [(154, vec![0], "unavailable"), (153, body, "unsupported")] {
        let report = view(&bank(version, bytes))?;
        let source = report
            .pointer("/objects/0/source_fields")
            .ok_or("Blend source fields")?;
        assert_eq!(source.pointer("/layout/value"), Some(&json!(layout)));
        assert_eq!(source.get("rows"), Some(&json!([])));
    }
    Ok(())
}

fn exact_word(source: &Value, field: &Value, bank: &[u8]) -> TestResult {
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&Value::Null),
        "standalone lineage"
    );
    assert_eq!(
        field.get("sourceSetComplete"),
        Some(&json!(true)),
        "the original word has a complete receipt"
    );
    let id = field.pointer("/sourceUsesRefs/0").ok_or("direct receipt")?;
    let uses = source
        .pointer("/source_inventory/uses")
        .and_then(Value::as_array)
        .ok_or("receipt inventory")?;
    let receipt = uses
        .iter()
        .find(|entry| entry.get("id") == Some(id))
        .ok_or("point receipt")?;
    let offset = |index: &str| -> Result<usize, Box<dyn std::error::Error>> {
        Ok(receipt
            .pointer(index)
            .and_then(Value::as_str)
            .ok_or("point offset")?
            .parse()?)
    };
    assert_eq!(
        bank.get(offset("/location/indices/0")?..offset("/location/indices/1")?),
        Some(NONFINITE.to_le_bytes().as_slice()),
        "the receipt selects the original crossfade word"
    );
    Ok(())
}

fn view(bank: &TestBank) -> Result<Value, Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = write_bank(directory.path(), bank)?;
    list_json(&["--id", &OBJECT_ID.to_string()], &path)
}

fn bank(version: u32, body: Vec<u8>) -> TestBank {
    TestBank::new(19)
        .version(version)
        .object(TestHircObject::event(1, &[]))
        .object(TestHircObject::raw(9, OBJECT_ID, body))
}

fn children() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut body = plain_node_prefix()?;
    words(&mut body, [2, CHILD_A, CHILD_B]);
    Ok(body)
}

fn initial_curve(body: &mut Vec<u8>) {
    body.extend(1_u16.to_le_bytes());
    words(body, [0xDABC_0011]);
    body.extend([0xF7, 0xE6, 0x02]);
    words(body, [CURVE]);
    body.push(0xDC);
    body.extend(1_u16.to_le_bytes());
    words(body, [0x3F00_0000, 0x4000_0000, 6]);
}

fn words<const N: usize>(body: &mut Vec<u8>, words: [u32; N]) {
    body.extend(words.into_iter().flat_map(u32::to_le_bytes));
}

fn crossfade(body: &mut Vec<u8>, id: u32, kind: u8, count: u32) {
    body.extend(id.to_le_bytes());
    body.push(kind);
    body.extend(count.to_le_bytes());
}
