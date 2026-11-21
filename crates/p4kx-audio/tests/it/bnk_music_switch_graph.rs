// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{BankHircSemantic, BankMusicSwitchRead, BankMusicSwitchRow, HircValue, Soundbank};
use p4kx_testkit::{TestBank, TestHircObject};

use super::{NODE_BYTES, SWITCH, TARGET, TestResult, open, parts};

const AMPLIFIED_DEPTH: u8 = 16;
const AMPLIFIED_NODES: usize = 1 + 2 * AMPLIFIED_DEPTH as usize;

#[test]
fn an_invalid_branch_behind_an_expanding_prefix_remains_malformed() -> TestResult {
    check_malformed_branch(u32::try_from(AMPLIFIED_NODES)?)
}

#[test]
fn an_ancestor_cycle_behind_an_expanding_prefix_remains_malformed() -> TestResult {
    check_malformed_branch(0)
}

fn check_malformed_branch(first_child: u32) -> TestResult {
    let mut bank = TestBank::new(1)
        .version(154)
        .object(TestHircObject::music_switch_with_shared_layers(
            SWITCH,
            AMPLIFIED_DEPTH,
            TARGET,
        ))
        .bytes();
    let tree_start = bank.len() - AMPLIFIED_NODES * NODE_BYTES;
    let link = tree_start + 2 * NODE_BYTES + size_of::<u32>();
    let end = link + size_of::<u32>();
    let invalid_link = (1 << u16::BITS) | first_child;
    bank.get_mut(link..end)
        .ok_or("the root's second child link")?
        .copy_from_slice(&invalid_link.to_le_bytes());

    let eager = Soundbank::parse(&bank)?;
    assert!(
        matches!(
            &eager.objects().first().ok_or("eager MusicSwitch")?.value,
            HircValue::Malformed(_)
        ),
        "the compact projection cannot hide a malformed child reference"
    );
    let mut source = open(&bank)?;
    let (header, mut hirc) = parts(&mut source)?;
    let Some(BankHircSemantic::MusicSwitch(mut switch)) =
        hirc.next_semantic(&header).transpose()?
    else {
        return Err("observed MusicSwitch".into());
    };
    assert!(
        matches!(switch.status, BankMusicSwitchRead::Malformed(_)),
        "the invalid branch remains explicit even after a large earlier subtree"
    );
    assert!(
        switch.fields.targets_compact,
        "skip amplified route details"
    );
    let rows = switch.decision_rows().collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        rows.len(),
        AMPLIFIED_NODES + usize::from(AMPLIFIED_DEPTH),
        "all groups and physical nodes, no route expansion"
    );
    assert!(
        rows.iter().any(|row| matches!(row,
            BankMusicSwitchRow::Node { ordinal: 2, value } if value.link.value == invalid_link)),
        "the original invalid packed link remains recoverable"
    );
    Ok(())
}

#[test]
fn ancestor_citations_can_require_compact_rows_with_an_expanded_target_summary() -> TestResult {
    const DEPTH: u32 = 16;
    const LEAVES: u32 = 32;
    let bank = TestBank::new(1)
        .version(154)
        .object(deep_prefix(DEPTH, LEAVES))
        .bytes();
    let eager = Soundbank::parse(&bank)?;
    let HircValue::MusicSwitch(summary) = &eager.objects().first().ok_or("MusicSwitch")?.value
    else {
        return Err("eager MusicSwitch".into());
    };
    assert!(
        !summary.targets_compact,
        "the ID list requires no ancestor receipts"
    );
    assert_eq!(
        summary.targets, [TARGET; LEAVES as usize],
        "all 32 original leaf IDs"
    );

    let mut source = open(&bank)?;
    let (header, mut hirc) = parts(&mut source)?;
    let Some(BankHircSemantic::MusicSwitch(mut switch)) =
        hirc.next_semantic(&header).transpose()?
    else {
        return Err("positioned MusicSwitch".into());
    };
    assert!(
        switch.fields.tree_complete,
        "the complete decision graph is valid"
    );
    assert!(
        switch.fields.targets_compact,
        "each leaf would repeat the shared chain of selectors"
    );
    let rows = switch.decision_rows().collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        rows.len(),
        usize::try_from(2 * DEPTH + LEAVES)?,
        "all argument and original node rows"
    );
    assert!(
        rows.iter()
            .all(|row| !matches!(row, BankMusicSwitchRow::Target { .. })),
        "the compact graph does not expand ancestor citations"
    );
    Ok(())
}

fn deep_prefix(depth: u32, leaves: u32) -> TestHircObject {
    let groups: Vec<(u32, u8)> = (1..=depth).map(|id| (id, 0)).collect();
    let mut tree = Vec::new();
    for index in 0..depth + leaves {
        let link = match index {
            index if index + 1 < depth => (1 << u16::BITS) | (index + 1),
            index if index < depth => (leaves << u16::BITS) | depth,
            _ => TARGET,
        };
        tree.extend(index.to_le_bytes());
        tree.extend(link.to_le_bytes());
        tree.extend(50_u16.to_le_bytes());
        tree.extend(100_u16.to_le_bytes());
    }
    TestHircObject::music_switch_with_tree(SWITCH, &[TARGET], &groups, &tree)
}
