// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::HircLayout;
use p4kx_media::{MediaRecord, Status};
use serde_json::{Value, json};

use super::super::audio_bank_container_node_provider::fixture::Evidence;
use super::super::audio_bank_state_provider::verify_layout;
use super::{
    ARGUMENT, NODE, PROPERTY, SUMMARY, TestResult, dialogue, export_observed, header, layout, node,
    properties, row, words,
};

const NODES: [(u64, u32, u32, u16, u16); 5] = [
    (0, 0, 0x0002_0001, 0xDA00, 0xAB00),
    (1, 0xAB12_3333, 0x0001_0003, 8, 93),
    (2, 0, 0xCD12_5530, 13, 97),
    (3, 0xAB12_4444, 0xCD12_5530, 17, 91),
    (4, 0, 0xCD12_55AA, 0xE4, 0xE5),
];

#[test]
fn complete_dialogue_retains_ordered_arguments_raw_links_and_property_words() -> TestResult {
    let mut body = header(2);
    words(&mut body, [0xCA02_2010, 0xCB02_2011]);
    body.extend([1, 0xDA]);
    words(&mut body, [60]);
    body.push(1);
    for (_, key, link, weight, probability) in NODES {
        node(&mut body, key, link, weight, probability);
    }
    properties(&mut body);
    let source = dialogue(154, body);
    verify_layout(&source, HircLayout::Undecoded)?;
    export_observed(&source, true, false, |rows, evidence| {
        check_summary(rows, evidence)?;
        check_arguments(rows, evidence)?;
        check_nodes(rows, evidence)?;
        check_properties(rows, evidence)
    })
}

fn check_summary(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    let summary = row(rows, SUMMARY, &[1, 0])?;
    layout(summary.1, "decoded", Status::Complete);
    evidence.byte(summary, "observed/probability", "dialogueProbability", 93)?;
    evidence.word(
        summary,
        "observed/argument_count",
        "dialogueArgumentCount",
        2,
    )?;
    evidence.word(summary, "observed/tree_size", "dialogueTreeSize", 60)?;
    evidence.byte(summary, "observed/mode_code", "dialogueModeCode", 1)?;
    assert_eq!(
        summary.1.data.pointer("/payload/observed/mode"),
        Some(&json!("weighted")),
        "the original decision code selects weighted mode",
    );
    evidence.conversion(
        summary,
        "observed/mode",
        "observed/mode_code",
        "dialogueModeCode",
    )?;
    assert_eq!(
        summary.1.data.pointer("/payload/observed/complete_nodes"),
        Some(&json!(5)),
        "all authored physical node words fit",
    );
    Ok(())
}

fn check_arguments(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    let first = row(rows, ARGUMENT, &[1, 0, 0])?;
    evidence.word(first, "group_id", "dialogueGroupId", 0xCA02_2010)?;
    evidence.byte(first, "group_type_code", "dialogueGroupTypeCode", 1)?;
    evidence.index(first.0, 2, first.1, "group_id", "dialogueArgumentPosition")?;
    assert_eq!(
        first.1.data.pointer("/payload/group_type"),
        Some(&json!("state")),
        "the first planar type names a State group",
    );
    let second = row(rows, ARGUMENT, &[1, 0, 1])?;
    evidence.word(second, "group_id", "dialogueGroupId", 0xCB02_2011)?;
    evidence.byte(second, "group_type_code", "dialogueGroupTypeCode", 0xDA)?;
    assert_eq!(
        second.1.data.pointer("/payload/group_type"),
        Some(&json!("unknown")),
        "the second planar type has no recognized name",
    );
    Ok(())
}

fn check_nodes(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    let tree_start = tree_start(rows)?;
    for (ordinal, key, link, weight, probability) in NODES {
        let original = row(rows, NODE, &[1, 0, ordinal])?;
        evidence.word(original, "key", "dialogueNodeKey", key)?;
        evidence.word(original, "link_word", "dialogueNodeLinkWord", link)?;
        let link_start = tree_start + ordinal * 12 + 4;
        assert_eq!(
            original.1.data.pointer("/payload/link_word/range/start"),
            Some(&json!(link_start.to_string())),
            "duplicate link words retain distinct physical coordinates",
        );
        evidence.halfword(original, "weight", "dialogueNodeWeight", weight)?;
        evidence.halfword(
            original,
            "probability",
            "dialogueNodeProbability",
            probability,
        )?;
        evidence.index(original.0, 2, original.1, "key", "dialogueNodePosition")?;
        assert_eq!(
            original.1.data.pointer("/payload/link_interpretation"),
            Some(&json!("undetermined")),
            "the original word alone cannot establish a branch or target",
        );
        evidence.conversion(
            original,
            "link_interpretation",
            "link_word",
            "dialogueNodeLinkWord",
        )?;
    }
    Ok(())
}

fn check_properties(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    let scalar = row(rows, PROPERTY, &[1, 0, 0, 0])?;
    evidence.byte(scalar, "property_id", "nodePropertyId", 0xED)?;
    evidence.word(
        scalar,
        "words/scalar_bits",
        "nodePropertyScalarBits",
        0x7FC0_0003,
    )?;
    let ranged = row(rows, PROPERTY, &[1, 0, 1, 0])?;
    evidence.byte(ranged, "property_id", "nodePropertyId", 0xEC)?;
    evidence.word(
        ranged,
        "words/minimum_bits",
        "nodePropertyMinimumBits",
        0xFF80_0000,
    )?;
    evidence.word(
        ranged,
        "words/maximum_bits",
        "nodePropertyMaximumBits",
        0x0000_0000,
    )
}

fn tree_start(rows: &[MediaRecord]) -> TestResult<u64> {
    let summary = row(rows, SUMMARY, &[1, 0])?;
    let object = row(rows, "audio-bank-object", &[1, 0])?;
    let object_start: u64 = object
        .1
        .data
        .pointer("/payload/body_range/start")
        .and_then(Value::as_str)
        .ok_or("original body start")?
        .parse()?;
    let tree_start = object_start + 24;
    assert_eq!(
        summary.1.data.pointer("/payload/observed/tree_range/start"),
        Some(&json!(tree_start.to_string())),
        "the tree starts after the ID and the independently authored planar header",
    );
    Ok(tree_start)
}
