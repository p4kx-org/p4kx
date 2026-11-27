// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestHircObject};
use serde_json::{Value, json};

use super::{Error, TestResult, json as list_json, text as list_text, write_bank};

const OWNER: u32 = 11;
const TARGET: u32 = 0xe7d0_80f1;
const GROUPS: [(u32, u8); 2] = [(0xa900_0001, 0x7f), (0xa900_0002, 1)];
const SWITCH_TYPE: u8 = 12;

#[test]
fn shared_target_keeps_both_paths_and_every_original_music_switch_node() -> TestResult {
    let source = view(154, fixture(0x0001_0003))?;
    let decisions = source.get("decisions").ok_or("original decisions")?;
    for (path, value) in [
        ("/layout/value", json!("decoded")),
        ("/continue_playback_code/value", json!(1)),
        ("/group_count/value", json!(2)),
        ("/tree_size/value", json!(84)),
        ("/tree_mode_code/value", json!(0)),
        ("/rows/0/kind", json!("group")),
        ("/rows/0/group_id/value", json!(GROUPS[0].0)),
        ("/rows/0/type_code/value", json!(0x7f)),
        ("/rows/1/ordinal/value", json!(1)),
        ("/rows/1/group_id/value", json!(GROUPS[1].0)),
        ("/rows/7/link_word/value", json!(0)),
        ("/rows/8/ordinal/value", json!(6)),
        ("/rows/8/weight/value", json!(0xfedc)),
        ("/rows/8/probability/value", json!(0xbeef)),
    ] {
        assert_eq!(
            decisions.pointer(path),
            Some(&value),
            "original field {path}"
        );
    }
    assert!(
        decisions.get("targets_compact").is_none(),
        "expanded occurrences keep their existing wire shape"
    );
    check_targets(decisions, &[1, 2])
}

#[test]
fn malformed_branch_keeps_original_words_and_the_prior_decision_path() -> TestResult {
    let source = view(154, fixture(0x0001_0007))?;
    let decisions = source.get("decisions").ok_or("original decisions")?;
    assert_eq!(
        decisions.pointer("/layout/value"),
        Some(&json!("malformed")),
        "second branch points outside the tree"
    );
    assert_eq!(
        decisions.pointer("/rows/4/link_word/value"),
        Some(&json!(0x0001_0007)),
        "invalid original link survives"
    );
    assert_eq!(
        source.pointer("/transition_rules/layout/value"),
        Some(&json!("decoded")),
        "preceding transition rules were read completely"
    );
    assert_eq!(
        source.pointer("/stingers/0/trigger_id/value"),
        Some(&json!(0x5100_0001)),
        "preceding stinger remains visible"
    );
    check_targets(decisions, &[1])
}

#[test]
fn layered_music_switch_keeps_complete_physical_graph_without_expanding_routes() -> TestResult {
    const DEPTH: u8 = 16;
    let group_count = usize::from(DEPTH);
    let node_count = 1 + 2 * group_count;
    let directory = tempfile::tempdir()?;
    let switch = TestHircObject::music_switch_with_shared_layers(OWNER, DEPTH, TARGET);
    let bank = TestBank::new(21).version(154).object(switch);
    let path = write_bank(directory.path(), &bank)?;
    let report = list_json(&[], &path)?;
    let object = report.pointer("/objects/0").ok_or("MusicSwitch")?;
    assert_eq!(object["status"], "decoded", "shared graph is valid");
    let detail = object.get("detail").ok_or("MusicSwitch detail")?;
    assert_eq!(detail["targets_compact"], true, "compact target projection");
    assert!(detail.get("targets").is_none(), "no expanded eager list");

    let decisions = object
        .pointer("/source_fields/decisions")
        .ok_or("original MusicSwitch decisions")?;
    assert_eq!(decisions["layout"]["value"], "decoded", "complete graph");
    assert_eq!(
        decisions["targets_compact"]["value"], true,
        "source fields mark the target projection"
    );
    let rows = decisions["rows"].as_array().ok_or("decision rows")?;
    assert_eq!(
        rows.len(),
        group_count + node_count,
        "all groups and physical nodes, without path occurrences"
    );
    for (ordinal, node) in rows.iter().skip(group_count).enumerate() {
        assert_eq!(node["kind"], "node", "physical decision node {ordinal}");
        assert_eq!(node["ordinal"]["value"], ordinal, "physical node order");
    }
    let root = rows.get(group_count).ok_or("root node")?;
    let leaf = rows.last().ok_or("last leaf")?;
    assert_eq!(root["link_word"]["value"], 0x0002_0001_u32, "root link");
    assert_eq!(leaf["link_word"]["value"], TARGET, "original target ID");
    assert!(
        list_text(&[], &path)?.contains("targets in the compact decision graph (--json)"),
        "text identifies where to find the original graph"
    );
    Ok(())
}

#[test]
fn unread_obsolete_and_explicitly_empty_decisions_have_distinct_states() -> TestResult {
    for (version, owner, status, count) in [
        (
            154,
            TestHircObject::raw(SWITCH_TYPE, OWNER, []),
            "unavailable",
            None,
        ),
        (153, fixture(0x0001_0003), "unsupported", None),
        (
            154,
            TestHircObject::music_switch_with_tree(OWNER, &[], &[], &[]),
            "decoded",
            Some(json!(0)),
        ),
    ] {
        let source = view(version, owner)?;
        let decisions = source.get("decisions").ok_or("decision coverage")?;
        assert_eq!(
            decisions.pointer("/layout/value"),
            Some(&json!(status)),
            "{status}"
        );
        assert_eq!(
            decisions.pointer("/group_count/value"),
            count.as_ref(),
            "{status}"
        );
        assert_eq!(decisions.get("rows"), Some(&json!([])), "{status}");
    }
    Ok(())
}

fn check_targets(decisions: &Value, branches: &[u32]) -> TestResult {
    let rows = decisions["rows"].as_array().ok_or("decision rows")?;
    let (physical, targets) = rows
        .split_at_checked(9)
        .ok_or("original groups and nodes")?;
    for (index, node) in (0_u32..).zip(physical.iter().skip(2)) {
        assert_eq!(node.get("kind"), Some(&json!("node")), "node row {index}");
        assert_eq!(
            node.pointer("/ordinal/value"),
            Some(&json!(index)),
            "node {index}"
        );
    }
    assert_eq!(
        targets.len(),
        branches.len(),
        "each complete target occurrence"
    );
    for ((ordinal, target), branch) in (0_u32..).zip(targets).zip(branches) {
        for (path, value) in [
            ("/kind", json!("target")),
            ("/ordinal/value", json!(ordinal)),
            ("/node_index/value", json!(3)),
            ("/level/value", json!(2)),
            ("/target_id/value", json!(TARGET)),
            ("/ancestor_nodes/0/value", json!(0)),
            ("/ancestor_nodes/1/value", json!(branch)),
        ] {
            assert_eq!(
                target.pointer(path),
                Some(&value),
                "target {ordinal} {path}"
            );
        }
    }
    Ok(())
}

fn fixture(second_branch: u32) -> TestHircObject {
    let nodes: [(u32, u32, u16, u16); 7] = [
        (0, 0x0002_0001, 50, 100),
        (11, 0x0002_0003, 51, 101),
        (12, second_branch, 52, 102),
        (21, TARGET, 61, 111),
        (22, 0, 62, 112),
        (23, 0, 63, 113),
        (24, 0x0112_2334, 0xfedc, 0xbeef),
    ];
    let mut tree = Vec::new();
    for (key, link, weight, probability) in nodes {
        tree.extend(key.to_le_bytes());
        tree.extend(link.to_le_bytes());
        tree.extend(weight.to_le_bytes());
        tree.extend(probability.to_le_bytes());
    }
    TestHircObject::music_switch_with_tree(OWNER, &[TARGET], &GROUPS, &tree)
}

fn view(version: u32, object: TestHircObject) -> Result<Value, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let bank = TestBank::new(21).version(version).object(object);
    let path = write_bank(directory.path(), &bank)?;
    let mut report = list_json(&["--id", &OWNER.to_string()], &path)?;
    report
        .pointer_mut("/objects/0/source_fields")
        .map(Value::take)
        .ok_or_else(|| "original switch fields".into())
}
