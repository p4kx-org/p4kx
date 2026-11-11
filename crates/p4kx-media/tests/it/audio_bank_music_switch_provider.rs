// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, TestHircObject, test_key};
use serde_json::json;

use super::audio_bank_hirc_action_origins::{self as origins, Origins};
use super::support;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SWITCH: &str = "audio-bank-music-switch";
const GROUP: &str = "audio-bank-music-switch-group";
const NODE: &str = "audio-bank-music-switch-node";
const TARGET: &str = "audio-bank-music-switch-target";
const TARGET_ID: u32 = 0xe7d0_80f1;
const GROUP_CODE: u8 = 0x99;
const NODE_BYTES: usize = 12;
const NODE_COUNT: usize = 4;

#[test]
fn music_switch_export_keeps_original_tree_and_exact_target_sources() -> TestResult {
    let folder = tempfile::tempdir()?;
    let bank = fixture();
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("plain.txt", b"fixture".as_slice()))
        .entry(TestEntry::zstd("music.bnk", bank.as_slice()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let out = folder.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::Audio],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &out, &options)?)?;
    let base = out.join(&index.artifact_base);
    let rows = support::records(&out, &index, "audio.jsonl")?;
    let evidence = Origins::read(&base)?;
    let tree = bank.len() - NODE_COUNT * NODE_BYTES;
    let (_, switch) = row(&rows, SWITCH, &[1, 0])?;
    assert_eq!(
        switch.status,
        Status::Complete,
        "the complete known switch layout exports successfully"
    );
    assert_eq!(
        switch.data.pointer("/payload/group_count"),
        Some(&json!(1)),
        "the authored group count survives"
    );
    assert_eq!(
        switch.data.pointer("/payload/tree_mode_code"),
        Some(&json!(0)),
        "the raw decision mode code survives"
    );
    assert_eq!(
        switch.data.pointer("/payload/continue_playback_code"),
        Some(&json!(1)),
        "the raw continue playback code survives"
    );
    assert_eq!(
        switch.data.pointer("/payload/transition_rule_count"),
        Some(&json!(2)),
        "the authored transition count survives"
    );
    let raw = switch.source.raw.as_ref().ok_or("retained original bank")?;
    assert_eq!(
        fs::read(base.join(&raw.path))?,
        bank,
        "the complete original bank is retained"
    );
    check_groups_and_nodes(&rows, &evidence, tree)?;
    check_targets(&rows, &evidence, tree)?;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for record in rows {
        assert!(
            schema.is_valid(&serde_json::to_value(record)?),
            "the record satisfies the built-in schema"
        );
    }
    Ok(())
}

fn check_groups_and_nodes(rows: &[MediaRecord], evidence: &Origins, tree: usize) -> TestResult {
    let (ordinal, group) = row(rows, GROUP, &[1, 0, 0])?;
    assert_eq!(
        group.data.pointer("/payload/type_code"),
        Some(&json!(GROUP_CODE)),
        "the original group type code survives"
    );
    let uses = evidence.uses(ordinal, 1, "/data/payload/type_code")?;
    assert!(
        origins::has_exact(
            &uses,
            "musicSwitchGroupType",
            "payload",
            &(tree - 6..tree - 5)
        ),
        "group type cites its exact original byte"
    );
    let (_, node) = row(rows, NODE, &[1, 0, 3])?;
    assert_eq!(
        node.data.pointer("/payload/key"),
        Some(&json!(31)),
        "the unvisited physical node keeps its key"
    );
    assert_eq!(
        node.data.pointer("/payload/link_word"),
        Some(&json!(0x1234_5678)),
        "the unvisited node keeps the raw link word"
    );
    assert_eq!(
        node.data.pointer("/payload/weight"),
        Some(&json!(0xfedc)),
        "the unvisited node keeps the raw weight"
    );
    assert_eq!(
        node.data.pointer("/payload/probability"),
        Some(&json!(0xbeef)),
        "the unvisited node keeps the raw probability"
    );
    Ok(())
}

fn check_targets(rows: &[MediaRecord], evidence: &Origins, tree: usize) -> TestResult {
    for (ordinal, node) in [(0, 1), (1, 2)] {
        let (record, target) = row(rows, TARGET, &[1, 0, ordinal])?;
        assert_eq!(
            target.data.pointer("/payload/node_index"),
            Some(&json!(node)),
            "duplicate target IDs have separate physical occurrences"
        );
        assert_eq!(
            target.data.pointer("/payload/target_id"),
            Some(&json!(TARGET_ID)),
            "the original target ID survives"
        );
        assert_eq!(
            target.data.pointer("/payload/ancestor_nodes"),
            Some(&json!([0])),
            "the original root selects both occurrences"
        );
        let uses = evidence.uses(record, 1, "/data/payload/target_id")?;
        let link = tree + node * NODE_BYTES + 4;
        assert!(
            origins::has_exact(&uses, "musicSwitchTargetId", "payload", &(link..link + 4)),
            "the target cites its original ID word"
        );
        assert!(
            origins::has_exact(
                &uses,
                "musicSwitchChildLink",
                "selector",
                &(tree + 4..tree + 8)
            ),
            "the origin cites the original root child link"
        );
        let ancestry = evidence.uses(record, 1, "/data/payload/ancestor_nodes/0")?;
        assert!(
            origins::has_exact(
                &ancestry,
                "musicSwitchChildLink",
                "selector",
                &(tree + 4..tree + 8)
            ),
            "the origin cites the original root child link"
        );
    }
    Ok(())
}

fn fixture() -> Vec<u8> {
    let nodes: [(u32, u32, u16, u16); NODE_COUNT] = [
        (0, 0x0002_0001, 50, 100),
        (11, TARGET_ID, 51, 101),
        (21, TARGET_ID, 52, 102),
        (31, 0x1234_5678, 0xfedc, 0xbeef),
    ];
    let mut tree = Vec::new();
    for (key, link, weight, probability) in nodes {
        tree.extend(key.to_le_bytes());
        tree.extend(link.to_le_bytes());
        tree.extend(weight.to_le_bytes());
        tree.extend(probability.to_le_bytes());
    }
    TestBank::new(1)
        .object(TestHircObject::music_switch_with_tree(
            81,
            &[31, 31],
            &[(0xa900_0001, GROUP_CODE)],
            &tree,
        ))
        .bytes()
}

fn row<'a>(
    rows: &'a [MediaRecord],
    namespace: &str,
    indices: &[u64],
) -> TestResult<(usize, &'a MediaRecord)> {
    rows.iter()
        .enumerate()
        .find(|(_, row)| {
            row.subresource.namespace == namespace && row.subresource.indices == indices
        })
        .ok_or_else(|| "original MusicSwitch occurrence".into())
}
