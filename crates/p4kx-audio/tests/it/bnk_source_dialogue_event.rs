// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Cursor;

use p4kx_audio::{
    BankDialogueChildCoverage, BankDialogueEventRow, BankDialogueEventRows, BankDialogueKeyRole,
    BankDialogueNode, BankHircDialogueEvent, BankHircSemantic, BankSource,
};
use p4kx_testkit::{TestBank, TestHircObject};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const NODE_BYTES: u32 = 12;

#[test]
fn dialogue_preserves_overlapping_interpretations_and_physical_key_roles() -> TestResult {
    let bytes = fixture(3, &[(17, 0x0002_0000), (0, 2), (7, u32::MAX)], &[]);
    with_dialogue(&bytes, |mut dialogue| {
        let tree = dialogue.fields().tree.as_ref().ok_or("checked tree")?;
        let start = tree.range.start;
        assert_eq!(tree.declared_nodes(), 3, "whole declared nodes");
        assert_eq!(tree.complete_nodes(), 3, "admitted nodes");
        let mut rows = dialogue.rows();
        check_root(next_node(&mut rows, 0)?, start)?;
        let value = next_node(&mut rows, 1)?;
        assert_eq!(
            value.key_role,
            BankDialogueKeyRole::DefaultCandidate,
            "a non-root zero key can be a default on a verified route"
        );
        assert_eq!(
            value.child_candidate.ok_or("empty window")?.coverage,
            BankDialogueChildCoverage::Empty,
            "zero child count remains explicit"
        );
        let value = next_node(&mut rows, 2)?;
        assert_eq!(
            value.key_role,
            BankDialogueKeyRole::ValueCandidate,
            "a nonzero non-root key can match a condition"
        );
        assert_eq!(
            value.child_candidate.ok_or("out-of-range window")?.coverage,
            BankDialogueChildCoverage::OutsideDeclared,
            "large raw link words do not establish a valid child window"
        );
        assert_eq!(
            value.target_candidate, value.link_word,
            "the physical coverage does not change the possible target"
        );
        assert!(rows.next().is_none(), "physical replay ends at the tree");
        Ok(())
    })
}

fn check_root(value: BankDialogueNode, start: u64) -> TestResult {
    assert_eq!(value.key_role, BankDialogueKeyRole::Root, "physical root");
    let child = value.child_candidate.ok_or("possible child window")?;
    assert_eq!(
        (child.first.value, child.first.range),
        (0, start + 4..start + 6),
        "the low half-word includes the root itself as a possible child"
    );
    assert_eq!(
        (child.count.value, child.count.range),
        (2, start + 6..start + 8),
        "the high half-word is the original possible count"
    );
    assert_eq!(
        child.coverage,
        BankDialogueChildCoverage::Complete,
        "coverage describes physical windows even for a self-reference"
    );
    assert_eq!(child.admitted_count, 2, "both candidate children fit");
    assert_eq!(
        value.target_candidate, value.link_word,
        "the alternative target retains the complete original word and range"
    );
    Ok(())
}

#[test]
fn dialogue_window_coverage_uses_complete_children_and_respects_partial_words() -> TestResult {
    let partial = [0, 0, 0, 0, 0xab, 0xcd, 0xef];
    let bytes = fixture(3, &[(0, 0x0002_0000)], &partial);
    with_dialogue(&bytes, |mut dialogue| {
        let tree = dialogue.fields().tree.as_ref().ok_or("checked tree")?;
        assert_eq!(tree.declared_nodes(), 3, "whole declared nodes");
        assert_eq!(tree.complete_nodes(), 1, "one complete node");
        let range = tree.partial_node_range().ok_or("partial node extent")?;
        let mut rows = dialogue.rows();
        let value = next_node(&mut rows, 0)?;
        let child = value.child_candidate.ok_or("possible child window")?;
        assert_eq!(
            child.coverage,
            BankDialogueChildCoverage::Truncated,
            "window fits the declaration with a shorter physical prefix"
        );
        assert_eq!(child.admitted_count, 1, "partial nodes are not admitted");
        let value = next_node(&mut rows, 1)?;
        assert_eq!(
            value.range, range,
            "the link fragment remains in the extent"
        );
        assert!(value.link_word.is_none(), "no complete link word");
        assert!(value.child_candidate.is_none(), "no child candidate");
        assert!(value.target_candidate.is_none(), "no target candidate");
        assert!(
            value.weight.is_none(),
            "link fragments are not weight bytes"
        );
        assert!(rows.next().is_none(), "physical replay ends at the tree");
        Ok(())
    })
}

fn next_node(
    rows: &mut BankDialogueEventRows<'_, Cursor<&[u8]>>,
    expected: u32,
) -> TestResult<BankDialogueNode> {
    let Some(BankDialogueEventRow::Node { ordinal, value }) = rows.next().transpose()? else {
        return Err("physical dialogue node".into());
    };
    assert_eq!(ordinal, expected, "physical replay order");
    Ok(value)
}

fn fixture(declared_nodes: u32, complete: &[(u32, u32)], partial: &[u8]) -> Vec<u8> {
    let mut body = vec![93];
    body.extend(0_u32.to_le_bytes());
    body.extend((declared_nodes * NODE_BYTES).to_le_bytes());
    body.push(1);
    for &(key, link) in complete {
        body.extend(key.to_le_bytes());
        body.extend(link.to_le_bytes());
        body.extend(50_u16.to_le_bytes());
        body.extend(100_u16.to_le_bytes());
    }
    if partial.is_empty() {
        body.extend([0, 0]);
    } else {
        body.extend(partial);
    }
    TestBank::new(1)
        .version(154)
        .object(TestHircObject::raw(15, 200, body))
        .bytes()
}

fn with_dialogue(
    bytes: &[u8],
    check: impl FnOnce(BankHircDialogueEvent<'_, Cursor<&[u8]>>) -> TestResult,
) -> TestResult {
    let mut bank = BankSource::open(Cursor::new(bytes), u64::try_from(bytes.len())?)?;
    let mut sections = bank.section_cursor();
    let header = bank
        .next_section(&mut sections)
        .transpose()?
        .ok_or("BKHD")?;
    let section = bank
        .next_section(&mut sections)
        .transpose()?
        .ok_or("HIRC")?;
    let header = bank.read_observed_header_for(&header)?;
    let mut hirc = bank.hirc(&section)?;
    let Some(BankHircSemantic::DialogueEvent(dialogue)) =
        hirc.next_semantic_with_source_fields(&header).transpose()?
    else {
        return Err("observed DialogueEvent".into());
    };
    check(*dialogue)
}
