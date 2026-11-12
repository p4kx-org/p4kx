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
    assert_eq!(source["kind"], "dialogue-event", "typed source declaration");
    assert_eq!(
        source["layout"]["value"], "decoded",
        "checked physical fields fit"
    );
    assert_eq!(
        source["version"]["value"], 154,
        "observed BKHD selects the layout"
    );
    assert_eq!(
        source["fields"]["probability"]["value"], 0,
        "explicit authored zero"
    );
    assert_eq!(
        source["fields"]["mode_code"]["value"], 0xFE,
        "unknown authored code stays"
    );
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
    assert_eq!(rows[1]["key"]["value"], 0, "explicit default key word");
    assert_eq!(
        rows[1]["link_word"]["value"], LINK,
        "the original link stays uninterpreted"
    );
    assert_eq!(
        rows[2]["scalar_bits"]["value"], 0x7FC0_0003_u32,
        "raw property value"
    );
    assert_link_source(source, &rows[1]["link_word"], &bank.bytes())
}

fn assert_link_source(source: &Value, link: &Value, original: &[u8]) -> TestResult {
    assert_eq!(
        link["containsEncryptedSource"],
        Value::Null,
        "standalone P4K lineage is unknown"
    );
    assert_eq!(
        link["sourceSetComplete"], true,
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
        Some(&json!("dialogueNodeLinkWord")),
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
        Some(LINK.to_le_bytes().as_slice()),
        "physical link bytes"
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
