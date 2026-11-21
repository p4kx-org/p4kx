// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestHircObject};
use serde_json::{Value, json};

use super::{TestResult, json as list_json, write_bank};

const OBJECT_ID: u32 = 0xDAC1_2500;
const LINK: u32 = 0xC8A2_1300;

#[test]
fn dialogue_listing_retains_original_rows_and_exact_standalone_source_use() -> TestResult {
    let dir = tempfile::tempdir()?;
    let bank = bank(154, complete_body());
    let path = write_bank(dir.path(), &bank)?;
    let report = list_json(&["--id", &OBJECT_ID.to_string()], &path)?;
    let object = report
        .pointer("/objects/0")
        .ok_or("selected physical object")?;
    assert_eq!(
        object["status"], "undecoded",
        "eager playback coverage stays distinct"
    );
    let source = object
        .get("source_fields")
        .ok_or("DialogueEvent source fields")?;
    assert_original_header(source)?;
    let rows = source["rows"].as_array().ok_or("ordered original rows")?;
    let kinds: Vec<_> = rows.iter().filter_map(|row| row["kind"].as_str()).collect();
    assert_eq!(
        kinds,
        ["argument", "node", "property"],
        "physical list order"
    );
    assert_eq!(
        rows[0]["group_id"]["value"], 0xDE71_2001_u32,
        "original group ID"
    );
    assert_eq!(
        rows[0]["group_type_code"]["value"], 0xDD,
        "unknown authored argument type"
    );
    assert_eq!(
        rows[0]["group_type"]["value"], "unknown",
        "an unknown group type retains its original code and classification"
    );
    assert_eq!(rows[1]["key"]["value"], 0, "explicit default key word");
    assert_eq!(
        rows[1]["link_word"]["value"], LINK,
        "the original link stays uninterpreted"
    );
    assert_eq!(
        rows[2]["scalar_bits"]["value"], 0x7FC0_0003_u32,
        "raw property value"
    );
    assert_source_word(
        source,
        &rows[1]["link_word"],
        "dialogueNodeLinkWord",
        &bank.bytes(),
        &LINK.to_le_bytes(),
    )
}

fn assert_original_header(source: &Value) -> TestResult {
    assert_eq!(
        source.get("kind").and_then(Value::as_str),
        Some("dialogue-event"),
        "typed source declaration"
    );
    assert_eq!(
        source.pointer("/layout/value").and_then(Value::as_str),
        Some("decoded"),
        "checked physical fields fit"
    );
    assert_eq!(
        source.pointer("/version/value").and_then(Value::as_u64),
        Some(154),
        "observed BKHD selects the layout"
    );
    let fields = source.get("fields").ok_or("DialogueEvent header fields")?;
    assert_eq!(
        fields.pointer("/probability/value").and_then(Value::as_u64),
        Some(0),
        "explicit authored zero"
    );
    assert_eq!(
        fields.pointer("/mode_code/value").and_then(Value::as_u64),
        Some(0xFE),
        "unknown authored code stays"
    );
    assert_eq!(
        fields.pointer("/mode/value").and_then(Value::as_str),
        Some("unknown"),
        "unknown mode has its own disposition"
    );
    Ok(())
}

#[test]
fn dialogue_candidate_window_retains_its_unread_prefix_and_possible_target() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut body = vec![100];
    body.extend(0_u32.to_le_bytes());
    body.extend(36_u32.to_le_bytes());
    body.push(1);
    let root_link = 0x0002_0001_u32;
    for (key, link) in [(0xA0_u32, root_link), (0, 0)] {
        body.extend(key.to_le_bytes());
        body.extend(link.to_le_bytes());
        body.extend(10_u16.to_le_bytes());
        body.extend(90_u16.to_le_bytes());
    }
    let bank = bank(154, body);
    let path = write_bank(dir.path(), &bank)?;
    let report = list_json(&["--id", &OBJECT_ID.to_string()], &path)?;
    let source = report
        .pointer("/objects/0/source_fields")
        .ok_or("DialogueEvent source fields")?;
    assert_eq!(
        source["layout"]["value"], "malformed",
        "physical body is truncated"
    );
    assert_eq!(source["fields"]["mode"]["value"], "weighted", "known mode");
    let tree = &source["fields"]["tree"];
    assert_eq!(tree["declared_nodes"]["value"], 3, "original table length");
    assert_eq!(tree["complete_nodes"]["value"], 2, "readable whole nodes");
    assert_root_child_window(source, root_link, &bank.bytes())?;
    let default = &source["rows"][1];
    assert_eq!(
        default["key_role"]["value"], "default-candidate",
        "non-root zero key"
    );
    assert_eq!(
        default["child_candidate"]["coverage"]["value"], "empty",
        "zero count"
    );
    assert!(
        default.get("target_candidate").is_none(),
        "zero has no candidate target ID"
    );
    Ok(())
}

fn assert_root_child_window(source: &Value, root_link: u32, bytes: &[u8]) -> TestResult {
    let root = source.pointer("/rows/0").ok_or("physical root node")?;
    assert_eq!(
        root.pointer("/key_role/value").and_then(Value::as_str),
        Some("root"),
        "first physical node"
    );
    assert_eq!(
        root.pointer("/link_interpretation/value")
            .and_then(Value::as_str),
        Some("undetermined"),
        "no selected link"
    );
    assert_eq!(
        root.pointer("/target_candidate/value")
            .and_then(Value::as_u64),
        Some(u64::from(root_link)),
        "possible unchanged target"
    );
    let child = root
        .get("child_candidate")
        .ok_or("original child candidate")?;
    let first = child.get("first").ok_or("low half-word")?;
    let count = child.get("count").ok_or("high half-word")?;
    assert_eq!(
        first.get("value").and_then(Value::as_u64),
        Some(1),
        "low half-word"
    );
    assert_eq!(
        count.get("value").and_then(Value::as_u64),
        Some(2),
        "high half-word"
    );
    assert_eq!(
        child
            .pointer("/admitted_count/value")
            .and_then(Value::as_u64),
        Some(1),
        "readable prefix in this window"
    );
    assert_eq!(
        child.pointer("/coverage/value").and_then(Value::as_str),
        Some("truncated"),
        "candidate has a missing child"
    );
    assert_source_word(source, first, "dialogueNodeChildFirst", bytes, &[1, 0])?;
    assert_source_word(source, count, "dialogueNodeChildCount", bytes, &[2, 0])
}

fn assert_source_word(
    source: &Value,
    link: &Value,
    field: &str,
    original: &[u8],
    expected: &[u8],
) -> TestResult {
    assert_eq!(
        link.get("containsEncryptedSource"),
        Some(&Value::Null),
        "standalone P4K lineage is unknown"
    );
    assert_eq!(
        link.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "exact read in the observed bank"
    );
    let references = link
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("exact link receipt references")?;
    let [reference] = references.as_slice() else {
        return Err("the link comes from one physical span".into());
    };
    let uses = source
        .pointer("/source_inventory/uses")
        .and_then(Value::as_array)
        .ok_or("source inventory uses")?;
    let receipt = uses
        .iter()
        .find(|entry| entry.get("id") == Some(reference))
        .ok_or("source use identified by link marker")?;
    assert_eq!(
        receipt.pointer("/location/field"),
        Some(&json!(field)),
        "exact physical label"
    );
    let offsets = receipt
        .pointer("/location/indices")
        .and_then(Value::as_array)
        .ok_or("original source location")?;
    let parse = |index: usize| -> Result<usize, Box<dyn std::error::Error>> {
        Ok(offsets
            .get(index)
            .and_then(Value::as_str)
            .ok_or("decimal offset")?
            .parse()?)
    };
    assert_eq!(
        original.get(parse(0)?..parse(1)?),
        Some(expected),
        "physical word bytes"
    );
    Ok(())
}

#[test]
fn partial_and_unsupported_dialogue_coverage_preserve_separate_dispositions() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut truncated = vec![43];
    truncated.extend(u32::MAX.to_le_bytes());
    truncated.extend(0xE55A_1234_u32.to_le_bytes());
    for (version, expected, body) in [
        (154, "malformed", truncated),
        (153, "unsupported", complete_body()),
        (160, "unsupported", complete_body()),
    ] {
        let path = write_bank(dir.path(), &bank(version, body))?;
        let report = list_json(&["--id", &OBJECT_ID.to_string()], &path)?;
        let source = report
            .pointer("/objects/0/source_fields")
            .ok_or("coverage")?;
        assert_eq!(
            source["layout"]["value"], expected,
            "the selected version determines coverage"
        );
        if version == 154 {
            assert_eq!(
                source["fields"]["argument_count"]["value"],
                u32::MAX,
                "original count"
            );
            assert_eq!(
                source["rows"][0]["group_id"]["value"], 0xE55A_1234_u32,
                "complete prefix"
            );
            assert!(
                source["rows"][0].get("group_type_code").is_none(),
                "unreached type"
            );
        } else {
            assert_eq!(source["rows"], json!([]), "unsupported source remains raw");
        }
    }
    Ok(())
}

fn bank(version: u32, body: Vec<u8>) -> TestBank {
    TestBank::new(3)
        .version(version)
        .object(TestHircObject::event(1, &[]))
        .object(TestHircObject::raw(15, OBJECT_ID, body))
}

fn complete_body() -> Vec<u8> {
    let mut body = vec![0];
    body.extend(1_u32.to_le_bytes());
    body.extend(0xDE71_2001_u32.to_le_bytes());
    body.push(0xDD);
    body.extend(12_u32.to_le_bytes());
    body.push(0xFE);
    body.extend(0_u32.to_le_bytes());
    body.extend(LINK.to_le_bytes());
    body.extend(10_u16.to_le_bytes());
    body.extend(90_u16.to_le_bytes());
    body.extend([1, 0xFD]);
    body.extend(0x7FC0_0003_u32.to_le_bytes());
    body.push(0);
    body
}
