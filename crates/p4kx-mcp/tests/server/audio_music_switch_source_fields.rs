// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestEntry, TestHircObject};
use serde_json::{Value, json};

use crate::audio::serve;
use crate::support::{Install, Server, TestResult};

const PATH: &str = "Data/Sounds/wwise/TestSwitchDecisions.bnk";
const TOOL: &str = "list_soundbank_objects";
const OWNER: u32 = 11;
const GROUP: u32 = 17;
const TARGET: u32 = 0xe7d0_80f1;
const NODE_BYTES: usize = 12;
const LINK_OFFSET: usize = size_of::<u32>();
const ROOT_LINK: u32 = 0x0003_0001;

#[test]
fn music_switch_decisions_mcp_keep_encrypted_target_occurrences() -> TestResult {
    let targets = [TARGET, TARGET, 0];
    let original = TestBank::new(21)
        .version(154)
        .object(TestHircObject::music_switch(
            OWNER,
            &[TARGET],
            GROUP,
            &targets,
        ))
        .bytes();
    let tree_start = original.len() - (1 + targets.len()) * NODE_BYTES;
    let tree = original.get(tree_start..).ok_or("original tree")?.to_vec();
    let install = Install::new(vec![TestEntry::zstd(PATH, original).encrypted()])?;
    let mut server = serve(&install)?;
    let report = server.ok(TOOL, &json!({"soundbank": PATH, "id": OWNER}))?;
    validate_schema(&mut server, &report)?;
    check_decisions(&report, tree_start, &tree)
}

#[test]
fn compact_music_switch_graph_keeps_encrypted_projection_evidence() -> TestResult {
    const DEPTH: u8 = 16;
    let groups = usize::from(DEPTH);
    let nodes = 1 + 2 * groups;
    let original = TestBank::new(21)
        .version(154)
        .object(TestHircObject::music_switch_with_shared_layers(
            OWNER, DEPTH, TARGET,
        ))
        .bytes();
    let header_bytes = 2 * size_of::<u32>() + 2 * size_of::<u8>();
    let group_bytes = groups * (size_of::<u32>() + size_of::<u8>());
    let decision_start = original.len() - (header_bytes + group_bytes + nodes * NODE_BYTES);
    let decision_range = json!([decision_start.to_string(), original.len().to_string()]);
    let install = Install::new(vec![TestEntry::zstd(PATH, original).encrypted()])?;
    let mut server = serve(&install)?;
    let report = server.ok(TOOL, &json!({"soundbank": PATH, "id": OWNER}))?;
    validate_schema(&mut server, &report)?;
    let source = report
        .pointer("/objects/0/source_fields")
        .ok_or("original MusicSwitch")?;
    let marker = source
        .pointer("/decisions/targets_compact")
        .ok_or("sourced compact projection")?;
    assert_eq!(marker["value"], true, "compact graph retained");
    assert_eq!(marker["containsEncryptedSource"], true, "encrypted BNK");
    assert_eq!(marker["sourceSetComplete"], true, "checked decision block");
    let references = marker["sourceUsesRefs"]
        .as_array()
        .ok_or("compact projection source reference")?;
    let inventory = source
        .pointer("/source_inventory/uses")
        .and_then(Value::as_array)
        .ok_or("MusicSwitch source inventory")?;
    assert!(
        inventory.iter().any(|receipt| {
            receipt.get("id").is_some_and(|id| references.contains(id))
                && receipt["role"] == "conversion"
                && receipt.pointer("/location/field").and_then(Value::as_str)
                    == Some("musicSwitchTargetsCompact")
                && receipt.pointer("/location/indices") == Some(&decision_range)
        }),
        "compact projection cites the checked original inputs"
    );
    assert!(
        report.pointer("/objects/0/detail/targets").is_none(),
        "the expanded eager list is absent"
    );
    let rows = source["decisions"]["rows"].as_array().ok_or("graph rows")?;
    assert_eq!(rows.len(), groups + nodes, "physical graph");
    Ok(())
}

fn validate_schema(server: &mut Server, report: &Value) -> TestResult {
    let tool = server
        .tools()?
        .into_iter()
        .find(|tool| tool.get("name").and_then(Value::as_str) == Some(TOOL))
        .ok_or("soundbank objects tool")?;
    let schema = tool.get("outputSchema").ok_or("output schema")?;
    assert!(
        schema
            .pointer("/$defs/MusicNodeObjectSourceFields/properties/decisions")
            .is_some(),
        "decision tree property in the advertised output schema",
    );
    jsonschema::validator_for(schema)?
        .validate(report)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn check_decisions(report: &Value, tree_start: usize, tree: &[u8]) -> TestResult {
    let source = report
        .pointer("/objects/0/source_fields")
        .ok_or("original MusicSwitch fields")?;
    let decisions = source.get("decisions").ok_or("MusicSwitch decisions")?;
    let rows = decisions["rows"]
        .as_array()
        .ok_or("original decision rows")?;
    let [
        group,
        root,
        first,
        second,
        zero,
        first_target,
        second_target,
    ] = rows.as_slice()
    else {
        return Err("expected a group, four physical nodes and two target occurrences".into());
    };
    for (row, field, value) in [
        (decisions, "/layout/value", json!("decoded")),
        (group, "/group_id/value", json!(GROUP)),
        (root, "/link_word/value", json!(ROOT_LINK)),
        (first, "/ordinal/value", json!(1)),
        (second, "/ordinal/value", json!(2)),
        (zero, "/link_word/value", json!(0)),
    ] {
        assert_eq!(
            row.pointer(field),
            Some(&value),
            "original {field} in {row}"
        );
    }

    let inventory = source
        .pointer("/source_inventory/uses")
        .and_then(Value::as_array)
        .ok_or("original MusicSwitch receipts")?;
    for (index, target) in [first_target, second_target].into_iter().enumerate() {
        let node = index + 1;
        assert_eq!(target["kind"], "target", "physical occurrence {index}");
        assert_eq!(
            target.pointer("/node_index/value"),
            Some(&json!(node)),
            "the target keeps its physical occurrence"
        );
        assert_eq!(
            target.pointer("/ancestor_nodes/0/value"),
            Some(&json!(0)),
            "the root selects both leaves"
        );
        let value = target.get("target_id").ok_or("original target word")?;
        assert_eq!(value["value"], TARGET, "physical occurrence {index}");
        assert_eq!(value["containsEncryptedSource"], true, "encrypted BNK");
        assert_eq!(value["sourceSetComplete"], true, "complete target route");
        check_receipts(inventory, value, tree_start, tree, node)?;
    }
    Ok(())
}

fn check_receipts(
    inventory: &[Value],
    value: &Value,
    tree_start: usize,
    tree: &[u8],
    leaf_index: usize,
) -> TestResult {
    let references = value["sourceUsesRefs"]
        .as_array()
        .ok_or("target source references")?;
    for (field, role, node, word) in [
        ("musicSwitchChildLink", "selector", 0, ROOT_LINK),
        ("musicSwitchTargetId", "payload", leaf_index, TARGET),
    ] {
        let offset = node * NODE_BYTES + LINK_OFFSET;
        let end = offset + size_of::<u32>();
        let indices = json!([
            (tree_start + offset).to_string(),
            (tree_start + end).to_string()
        ]);
        assert!(
            inventory.iter().any(|receipt| {
                receipt.get("id").is_some_and(|id| references.contains(id))
                    && receipt["role"] == role
                    && receipt.pointer("/location/field").and_then(Value::as_str) == Some(field)
                    && receipt.pointer("/location/indices") == Some(&indices)
            }),
            "{field} {role} for original physical node {node}",
        );
        assert_eq!(
            tree.get(offset..end),
            Some(word.to_le_bytes().as_slice()),
            "original bytes for {field} at physical node {node}",
        );
    }
    Ok(())
}
