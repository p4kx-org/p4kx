// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Cursor;

use p4kx_audio::{
    BankHirc, BankHircSemantic, BankMusicSwitchRead, BankMusicSwitchRow, BankMusicSwitchRows,
    BankObservedHeader, BankSource, HircValue, Soundbank,
};
use p4kx_testkit::{TestBank, TestHircObject};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SWITCH: u32 = 81;
const CHILD: u32 = 0xa453_91c2;
const TARGET: u32 = 0xe7d0_80f1;
const GROUPS: [(u32, u8); 2] = [(0xa900_0001, 0x7f), (0xa900_0002, 1)];
const NODE_COUNT: usize = 7;
const NODE_BYTES: usize = 12;

#[test]
fn music_switch_keeps_physical_nodes_and_repeated_target_paths() -> TestResult {
    let bank = fixture(154, false);
    let slice = Soundbank::parse(bank.as_slice())?;
    let HircValue::MusicSwitch(legacy) = &slice.objects().first().ok_or("music switch")?.value
    else {
        return Err("legacy MusicSwitch".into());
    };
    assert_eq!(
        legacy.targets,
        [TARGET, TARGET],
        "the repeated physical leaf is visited twice"
    );
    let mut source = open(&bank)?;
    let (header, mut hirc) = parts(&mut source)?;
    let Some(BankHircSemantic::MusicSwitch(switch)) = hirc.next_semantic(&header).transpose()?
    else {
        return Err("checked MusicSwitch".into());
    };
    assert!(matches!(switch.status, BankMusicSwitchRead::Decoded));
    let range = switch
        .fields
        .tree_range
        .as_ref()
        .ok_or("original decision tree extent")?;
    let tree_start = u64::try_from(bank.len() - NODE_COUNT * NODE_BYTES)?;
    assert_eq!(range, &(tree_start..u64::try_from(bank.len())?));
    assert_eq!(
        switch.fields.group_count.as_ref().map(|count| count.value),
        Some(2)
    );
    assert_eq!(
        switch
            .fields
            .transitions
            .rule_count
            .as_ref()
            .map(|count| count.value),
        Some(2)
    );
    check_rows(switch.rows, tree_start, &[[0, 1], [0, 2]])?;
    Ok(())
}

#[test]
fn music_switch_keeps_raw_nodes_when_a_later_path_is_invalid() -> TestResult {
    let bank = fixture(154, true);
    let mut source = open(&bank)?;
    let (header, mut hirc) = parts(&mut source)?;
    let Some(BankHircSemantic::MusicSwitch(switch)) = hirc.next_semantic(&header).transpose()?
    else {
        return Err("physical malformed MusicSwitch".into());
    };
    assert!(matches!(switch.status, BankMusicSwitchRead::Malformed(_)));
    let tree_start = u64::try_from(bank.len() - NODE_COUNT * NODE_BYTES)?;
    check_rows(switch.rows, tree_start, &[[0, 1]])
}

#[test]
fn music_switch_keeps_other_bank_layouts_raw_or_unsupported() -> TestResult {
    for version in [153, 155] {
        let bank = fixture(version, false);
        let mut source = open(&bank)?;
        let (header, mut hirc) = parts(&mut source)?;
        match hirc
            .next_semantic(&header)
            .transpose()?
            .ok_or("physical switch")?
        {
            BankHircSemantic::Raw(_) if version == 153 => {}
            BankHircSemantic::MusicSwitch(switch) if version == 155 => {
                assert!(matches!(switch.status, BankMusicSwitchRead::Unsupported(_)));
                assert!(switch.fields.group_count.is_none());
                assert_eq!(switch.rows.count(), 0);
            }
            _ => return Err("wrong version-dependent MusicSwitch layout".into()),
        }
    }
    Ok(())
}

fn check_rows(
    mut rows: BankMusicSwitchRows<'_, Cursor<&[u8]>>,
    tree_start: u64,
    expected: &[[u32; 2]],
) -> TestResult {
    let mut groups = Vec::new();
    let mut physical = Vec::new();
    let mut paths = Vec::new();
    while let Some(row) = rows.next() {
        match row? {
            BankMusicSwitchRow::Group { value, .. } => {
                groups.push((
                    value.id.value,
                    value.type_code.ok_or("original group type")?.value,
                ));
            }
            BankMusicSwitchRow::Node { ordinal, value } => {
                if ordinal == 6 {
                    assert_eq!(value.weight.value, 0xfedc, "unreached physical node");
                    assert_eq!(value.probability.value, 0xbeef, "raw probability word");
                }
                physical.push(ordinal);
            }
            BankMusicSwitchRow::Target { ordinal, value } => {
                let route = rows.current_target_route().collect::<Vec<_>>();
                let [root, branch] = route.as_slice() else {
                    return Err("two original ancestor decisions".into());
                };
                assert_eq!(
                    usize::try_from(ordinal)?,
                    paths.len(),
                    "target ordinals follow traversal order"
                );
                assert_eq!(
                    root.link.range,
                    tree_start + 4..tree_start + 8,
                    "root range covers the original child link"
                );
                let at = tree_start + u64::from(branch.node_index) * NODE_BYTES as u64 + 4;
                assert_eq!(
                    branch.link.range,
                    at..at + 4,
                    "branch range covers the selected child link"
                );
                assert_eq!(
                    value.node_index, 3,
                    "same physical target node on both paths"
                );
                assert_eq!(value.level, 2, "target is two levels below its root");
                assert_eq!(
                    value.id.value, TARGET,
                    "target ID preserves the stored word"
                );
                paths.push([root.node_index, branch.node_index]);
            }
            BankMusicSwitchRow::Child { .. } | BankMusicSwitchRow::Stinger { .. } => {}
            _ => return Err("unexpected MusicSwitch row".into()),
        }
    }
    assert_eq!(groups, GROUPS, "original ordered group IDs and raw types");
    assert_eq!(physical, [0, 1, 2, 3, 4, 5, 6], "all physical nodes");
    assert_eq!(paths, expected, "complete readable traversal prefix");
    Ok(())
}

fn fixture(version: u32, bad_branch: bool) -> Vec<u8> {
    let last_branch = if bad_branch { 0x0001_0007 } else { 0x0001_0003 };
    let nodes: [(u32, u32, u16, u16); NODE_COUNT] = [
        (0, 0x0002_0001, 50, 100),
        (11, 0x0002_0003, 51, 101),
        (12, last_branch, 52, 102),
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
    TestBank::new(1)
        .version(version)
        .object(TestHircObject::music_switch_with_tree(
            SWITCH,
            &[CHILD, CHILD],
            &GROUPS,
            &tree,
        ))
        .bytes()
}

fn open(bank: &[u8]) -> TestResult<BankSource<Cursor<&[u8]>>> {
    Ok(BankSource::open(
        Cursor::new(bank),
        u64::try_from(bank.len())?,
    )?)
}

fn parts<'bank, 'bytes>(
    bank: &'bank mut BankSource<Cursor<&'bytes [u8]>>,
) -> TestResult<(BankObservedHeader, BankHirc<'bank, Cursor<&'bytes [u8]>>)> {
    let mut sections = bank.section_cursor();
    let header = bank
        .next_section(&mut sections)
        .transpose()?
        .ok_or("BKHD")?;
    let hirc = bank
        .next_section(&mut sections)
        .transpose()?
        .ok_or("HIRC")?;
    Ok((bank.read_observed_header_for(&header)?, bank.hirc(&hirc)?))
}
