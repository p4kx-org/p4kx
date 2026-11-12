// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::Status;
use p4kx_testkit::TestHircObject;
use serde_json::json;

use super::audio_bank_container_node_provider::fixture::{bank, export_observed, layout};
use super::audio_bank_state_provider::row;
use super::support::audio_provider::TestResult;

const SUMMARY: &str = "audio-bank-dialogue-event";
const ARGUMENT: &str = "audio-bank-dialogue-argument";
const NODE: &str = "audio-bank-dialogue-node";
const PROPERTY: &str = "audio-bank-hirc-node-property";

#[path = "audio_bank_dialogue_event_complete.rs"]
mod complete;

#[test]
fn impossible_argument_count_retains_only_checked_group_ids() -> TestResult {
    let mut body = header(u32::MAX);
    words(&mut body, [0xCD02_1234]);
    body.extend([0xF1, 0xF2]);
    export_observed(&dialogue(154, body), false, true, |rows, evidence| {
        let summary = row(rows, SUMMARY, &[1, 0])?;
        layout(summary.1, "malformed", Status::Partial);
        evidence.word(
            summary,
            "observed/argument_count",
            "dialogueArgumentCount",
            u32::MAX,
        )?;
        assert_eq!(
            summary
                .1
                .data
                .pointer("/payload/observed/checked_arguments"),
            Some(&json!(1))
        );
        assert!(
            summary
                .1
                .data
                .pointer("/payload/observed/tree_size")
                .is_none()
        );
        let group = row(rows, ARGUMENT, &[1, 0, 0])?;
        evidence.word(group, "group_id", "dialogueGroupId", 0xCD02_1234)?;
        assert!(group.1.data.pointer("/payload/group_type_code").is_none());
        assert_eq!(
            rows.iter()
                .filter(|row| row.subresource.namespace == ARGUMENT)
                .count(),
            1
        );
        Ok(())
    })
}

#[test]
fn impossible_tree_length_retains_checked_node_prefix_without_properties() -> TestResult {
    let mut body = header(0);
    words(&mut body, [u32::MAX]);
    body.push(0xFC);
    words(&mut body, [0xBCAD_0001, 0xCA00_5544]);
    body.push(0xAB);
    export_observed(&dialogue(154, body), true, true, |rows, evidence| {
        let summary = row(rows, SUMMARY, &[1, 0])?;
        layout(summary.1, "malformed", Status::Partial);
        evidence.word(summary, "observed/tree_size", "dialogueTreeSize", u32::MAX)?;
        evidence.byte(summary, "observed/mode_code", "dialogueModeCode", 0xFC)?;
        assert_eq!(
            summary.1.data.pointer("/payload/observed/mode"),
            Some(&json!("unknown"))
        );
        assert_eq!(
            summary.1.data.pointer("/payload/observed/tree_coverage"),
            Some(&json!("truncated"))
        );
        assert_eq!(
            summary.1.data.pointer("/payload/observed/complete_nodes"),
            Some(&json!(0))
        );
        let partial = row(rows, NODE, &[1, 0, 0])?;
        layout(partial.1, "malformed", Status::Partial);
        evidence.word(partial, "key", "dialogueNodeKey", 0xBCAD_0001)?;
        evidence.word(partial, "link_word", "dialogueNodeLinkWord", 0xCA00_5544)?;
        assert!(partial.1.data.pointer("/payload/weight").is_none());
        assert!(rows.iter().all(|row| row.subresource.namespace != PROPERTY));
        Ok(())
    })
}

#[test]
fn misaligned_tree_keeps_complete_properties_at_the_declared_boundary() -> TestResult {
    let mut body = header(0);
    words(&mut body, [14]);
    body.push(0);
    node(&mut body, 0, 0xCBA0_5530, 19, 86);
    body.extend([0xEA, 0xEB]);
    properties(&mut body);
    let source = dialogue(154, body);
    export_observed(&source, false, true, |rows, evidence| {
        let summary = row(rows, SUMMARY, &[1, 0])?;
        layout(summary.1, "malformed", Status::Partial);
        assert_eq!(
            summary.1.data.pointer("/payload/observed/tree_coverage"),
            Some(&json!("misaligned"))
        );
        assert_eq!(
            summary.1.data.pointer("/payload/observed/complete_nodes"),
            Some(&json!(1))
        );
        assert_eq!(
            summary
                .1
                .data
                .pointer("/payload/observed/partial_node_bytes"),
            Some(&json!(2))
        );
        assert_eq!(
            summary
                .1
                .data
                .pointer("/payload/observed/properties/complete"),
            Some(&json!(true))
        );
        layout(row(rows, NODE, &[1, 0, 0])?.1, "decoded", Status::Complete);
        assert_eq!(
            super::audio_bank_state_provider::source_span(
                &source,
                &summary.1.data,
                "/payload/observed/partial_node_range",
            )?,
            &[0xEA, 0xEB],
            "the physical tree tail ends before the first property count",
        );
        assert_eq!(
            rows.iter()
                .filter(|row| row.subresource.namespace == NODE)
                .count(),
            1
        );
        let ranged = row(rows, PROPERTY, &[1, 0, 1, 0])?;
        evidence.word(
            ranged,
            "words/minimum_bits",
            "nodePropertyMinimumBits",
            0xFF80_0000,
        )
    })
}

#[test]
fn empty_unknown_mode_and_older_banks_remain_distinct() -> TestResult {
    let mut body = header(0);
    words(&mut body, [0]);
    body.extend([0xFE, 0, 0]);
    for (version, expected) in [(154, "decoded"), (153, "unsupported")] {
        let source = dialogue(version, body.clone());
        export_observed(&source, false, false, |rows, evidence| {
            let summary = row(rows, SUMMARY, &[1, 0])?;
            layout(summary.1, expected, Status::Complete);
            if version == 154 {
                evidence.byte(summary, "observed/mode_code", "dialogueModeCode", 0xFE)?;
                assert_eq!(
                    summary.1.data.pointer("/payload/observed/mode"),
                    Some(&json!("unknown"))
                );
                assert_eq!(
                    summary.1.data.pointer("/payload/observed/tree_coverage"),
                    Some(&json!("complete"))
                );
                assert_eq!(
                    summary.1.data.pointer("/payload/observed/complete_nodes"),
                    Some(&json!(0))
                );
            } else {
                assert!(summary.1.data.pointer("/payload/observed").is_none());
            }
            assert!(rows.iter().all(|row| !matches!(
                row.subresource.namespace.as_str(),
                ARGUMENT | NODE | PROPERTY
            )));
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn truncated_type_table_keeps_both_ids_and_only_the_first_type() -> TestResult {
    let mut body = header(2);
    words(&mut body, [0xDE0A_1110, 0xDE0A_1111]);
    body.push(0);
    export_observed(&dialogue(154, body), true, true, |rows, evidence| {
        let summary = row(rows, SUMMARY, &[1, 0])?;
        layout(summary.1, "malformed", Status::Partial);
        assert_eq!(
            summary
                .1
                .data
                .pointer("/payload/observed/checked_arguments"),
            Some(&json!(2))
        );
        assert_eq!(
            summary
                .1
                .data
                .pointer("/payload/observed/checked_argument_types"),
            Some(&json!(1))
        );
        let first = row(rows, ARGUMENT, &[1, 0, 0])?;
        evidence.word(first, "group_id", "dialogueGroupId", 0xDE0A_1110)?;
        evidence.byte(first, "group_type_code", "dialogueGroupTypeCode", 0)?;
        evidence.conversion(
            first,
            "group_type",
            "group_type_code",
            "dialogueGroupTypeCode",
        )?;
        assert_eq!(
            first.1.data.pointer("/payload/group_type"),
            Some(&json!("switch"))
        );
        let second = row(rows, ARGUMENT, &[1, 0, 1])?;
        evidence.word(second, "group_id", "dialogueGroupId", 0xDE0A_1111)?;
        assert!(second.1.data.pointer("/payload/group_type_code").is_none());
        assert!(
            summary
                .1
                .data
                .pointer("/payload/observed/tree_size")
                .is_none()
        );
        Ok(())
    })
}

#[test]
fn truncated_ranged_bundle_keeps_complete_tree_and_original_minimum() -> TestResult {
    let mut body = header(0);
    words(&mut body, [12]);
    body.push(0);
    node(&mut body, 0, 0xCE12_5530, 19, 86);
    body.extend([0, 1, 0xEC]);
    words(&mut body, [0xFF80_0000]);
    body.extend([0xA0, 0xB0, 0xC0]);
    export_observed(&dialogue(154, body), false, true, |rows, evidence| {
        let summary = row(rows, SUMMARY, &[1, 0])?;
        layout(summary.1, "malformed", Status::Partial);
        assert_eq!(
            summary.1.data.pointer("/payload/observed/tree_coverage"),
            Some(&json!("complete"))
        );
        assert_eq!(
            summary
                .1
                .data
                .pointer("/payload/observed/ranged_modifiers/complete"),
            Some(&json!(false))
        );
        layout(row(rows, NODE, &[1, 0, 0])?.1, "decoded", Status::Complete);
        let ranged = row(rows, PROPERTY, &[1, 0, 1, 0])?;
        evidence.word(
            ranged,
            "words/minimum_bits",
            "nodePropertyMinimumBits",
            0xFF80_0000,
        )?;
        assert!(
            ranged
                .1
                .data
                .pointer("/payload/words/maximum_bits")
                .is_none()
        );
        Ok(())
    })
}

fn dialogue(version: u32, body: Vec<u8>) -> Vec<u8> {
    bank(version, TestHircObject::raw(15, 0xDA10_0510, body))
}

fn header(argument_count: u32) -> Vec<u8> {
    let mut body = vec![93];
    words(&mut body, [argument_count]);
    body
}

fn words<const N: usize>(body: &mut Vec<u8>, words: [u32; N]) {
    body.extend(words.into_iter().flat_map(u32::to_le_bytes));
}

fn node(body: &mut Vec<u8>, key: u32, link: u32, weight: u16, probability: u16) {
    words(body, [key, link]);
    body.extend(weight.to_le_bytes());
    body.extend(probability.to_le_bytes());
}

fn properties(body: &mut Vec<u8>) {
    body.extend([1, 0xED]);
    words(body, [0x7FC0_0003]);
    body.extend([1, 0xEC]);
    words(body, [0xFF80_0000, 0]);
}
