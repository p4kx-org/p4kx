// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestEntry, TestHircObject};
use serde_json::{Value, json};

use crate::audio::serve;
use crate::support::{Install, TestResult};

const BANK: &str = "Data/Sounds/wwise/Dialogue.bnk";
const DIALOGUE: u32 = 17;

#[test]
fn encrypted_dialogue_mcp_binds_original_fields_to_the_actual_bank() -> TestResult {
    let bytes = TestBank::new(5)
        .version(154)
        .object(TestHircObject::raw(15, DIALOGUE, dialogue_body(1)))
        .bytes();
    let install = Install::new(vec![TestEntry::zstd(BANK, bytes).encrypted()])?;
    let mut server = serve(&install)?;
    let response = server.ok(
        "list_soundbank_objects",
        &json!({"soundbank": BANK, "id": DIALOGUE}),
    )?;
    let source = response
        .pointer("/objects/0/source_fields")
        .ok_or("typed DialogueEvent")?;
    let link = &source["rows"][0]["link_word"];
    assert_eq!(link["value"], 0xA152_1010_u32, "exact link");
    assert_eq!(
        link["containsEncryptedSource"], true,
        "actual encrypted payload"
    );
    assert_eq!(link["sourceSetComplete"], true, "known contributing set");
    let reference = link["sourceUsesRefs"]
        .as_array()
        .and_then(|refs| refs.first())
        .ok_or("direct link receipt")?;
    let uses = source["source_inventory"]["uses"]
        .as_array()
        .ok_or("inventory uses")?;
    let observed = uses
        .iter()
        .find(|used| &used["id"] == reference)
        .ok_or("original link use")?;
    assert_eq!(
        observed["location"]["field"], "dialogueNodeLinkWord",
        "physical field label"
    );
    assert_eq!(observed["scope"], "payload", "payload scope");
    assert_eq!(
        source["version"]["containsEncryptedSource"], true,
        "actual BKHD payload"
    );
    assert_encrypted_candidates(&source["rows"][0], uses)?;
    let tool = server
        .tools()?
        .into_iter()
        .find(|tool| tool["name"] == "list_soundbank_objects")
        .ok_or("soundbank objects tool")?;
    assert_eq!(
        tool.pointer("/outputSchema/$defs/DialogueObjectSourceFields/properties/rows/type"),
        Some(&json!("array")),
        "the MCP tool describes the shared streamed source row array",
    );
    Ok(())
}

fn assert_encrypted_candidates(node: &Value, uses: &[Value]) -> TestResult {
    assert_eq!(
        node.pointer("/key_role/value").and_then(Value::as_str),
        Some("root"),
        "root has no argument condition"
    );
    assert_eq!(
        node.pointer("/target_candidate/value")
            .and_then(Value::as_u64),
        Some(0xA152_1010),
        "possible target"
    );
    let child = node
        .get("child_candidate")
        .ok_or("original child candidate")?;
    assert_eq!(
        child.pointer("/first/value").and_then(Value::as_u64),
        Some(0x1010),
        "candidate child start"
    );
    assert_eq!(
        child.pointer("/count/value").and_then(Value::as_u64),
        Some(0xA152),
        "candidate child count"
    );
    assert_eq!(
        child
            .pointer("/admitted_count/value")
            .and_then(Value::as_u64),
        Some(0),
        "candidate outside physical table"
    );
    let coverage = child.get("coverage").ok_or("checked child coverage")?;
    assert_encrypted_coverage(coverage, uses)
}

fn assert_encrypted_coverage(coverage: &Value, uses: &[Value]) -> TestResult {
    assert_eq!(
        coverage.get("value").and_then(Value::as_str),
        Some("outside-declared"),
        "declared table boundary"
    );
    assert_eq!(
        coverage
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "actual encrypted source"
    );
    assert_eq!(
        coverage.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "all coverage inputs are observed"
    );
    let refs = coverage
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("coverage source uses")?;
    let mut labels = refs
        .iter()
        .map(|id| {
            uses.iter()
                .find(|source| source.get("id") == Some(id))
                .and_then(|source| source.pointer("/location/field"))
                .and_then(Value::as_str)
                .ok_or("original coverage field")
        })
        .collect::<Result<Vec<_>, _>>()?;
    labels.sort_unstable();
    assert_eq!(
        labels,
        [
            "dialogueNodeLinkWord",
            "dialogueTreeExtent",
            "dialogueTreeSize"
        ],
        "the checked coverage refers to the link and both tree bounds",
    );
    Ok(())
}

#[test]
fn a_large_sourced_object_keeps_whole_object_paging_and_an_explicit_transport_error() -> TestResult
{
    let bytes = TestBank::new(5)
        .version(154)
        .object(TestHircObject::event(1, &[]))
        .object(TestHircObject::raw(15, DIALOGUE, dialogue_body(4_096)))
        .object(TestHircObject::event(2, &[]))
        .bytes();
    let install = Install::new(vec![TestEntry::stored(BANK, bytes)])?;
    let mut server = serve(&install)?;
    let listing = server.ok("list_soundbank_objects", &json!({"soundbank": BANK}))?;
    let shown = listing["objects"].as_array().ok_or("first page objects")?;
    assert_eq!(
        shown.len(),
        1,
        "the transport admits whole preceding objects"
    );
    assert_eq!(shown[0]["id"], 1, "physical first object");
    let cursor = listing["next_cursor"]
        .as_str()
        .ok_or("continuation at large object")?;
    let refused = server.error(
        "list_soundbank_objects",
        &json!({"soundbank": BANK, "cursor": cursor}),
    )?;
    assert!(
        refused.contains("object 17") && refused.contains("no filter narrows it further"),
        "the complete source fields exceed the existing response budget: {refused}",
    );
    let later = server.ok(
        "list_soundbank_objects",
        &json!({"soundbank": BANK, "id": 2}),
    )?;
    assert_eq!(
        later["objects"][0]["id"], 2,
        "later objects remain independently accessible"
    );
    Ok(())
}

fn dialogue_body(node_count: u32) -> Vec<u8> {
    let mut body = vec![100];
    body.extend(0_u32.to_le_bytes());
    body.extend((12 * node_count).to_le_bytes());
    body.push(0);
    for index in 0..node_count {
        body.extend(index.to_le_bytes());
        body.extend(0xA152_1010_u32.to_le_bytes());
        body.extend(2_u16.to_le_bytes());
        body.extend(100_u16.to_le_bytes());
    }
    body.extend([0, 0]);
    body
}
