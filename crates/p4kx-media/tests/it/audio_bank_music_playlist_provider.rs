// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord, Status};
use p4kx_testkit::{
    ArchiveBuilder, TestBank, TestEntry, TestHircObject, TestPlaylistItem, test_key,
};
use serde_json::json;

use super::audio_bank_hirc_action_origins::{self as origins, Origins};
use super::support;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const PLAYLIST: &str = "audio-bank-music-playlist";
const ITEM: &str = "audio-bank-music-playlist-item";
const CHILD: &str = "audio-bank-music-playlist-child";
const STINGER: &str = "audio-bank-music-playlist-stinger";
const SEGMENT: u32 = 0xa453_91c2;
const MODE: u32 = 0xe7d0_80f1;

#[test]
fn music_playlist_export_keeps_raw_items_node_rows_and_transition_range() -> TestResult {
    let folder = tempfile::tempdir()?;
    let bank = fixture()?;
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
    let records = support::records(&out, &index, "audio.jsonl")?;
    let (ordinal, item) = row(&records, ITEM, &[1, 0, 1])?;
    check_item(item);
    let evidence = Origins::read(&base)?;
    check_flags(&evidence, ordinal, bank.len())?;
    let (_, playlist) = row(&records, PLAYLIST, &[1, 0])?;
    assert_eq!(playlist.status, Status::Complete);
    assert_eq!(
        playlist.data.pointer("/payload/item_count"),
        Some(&json!(2))
    );
    assert_eq!(
        playlist.data.pointer("/payload/transition_rule_count"),
        Some(&json!(2))
    );
    assert_eq!(
        playlist
            .data
            .pointer("/payload/transition_rules_range/end_exclusive"),
        Some(&json!((bank.len() - 64).to_string())),
        "the transition extent ends before the item count"
    );
    let raw = playlist
        .source
        .raw
        .as_ref()
        .ok_or("retained original bank")?;
    assert_eq!(fs::read(base.join(&raw.path))?, bank);
    let (_, child) = row(&records, CHILD, &[1, 0, 1])?;
    assert_eq!(
        child.data.pointer("/payload/child_id"),
        Some(&json!(SEGMENT))
    );
    let (_, stinger) = row(&records, STINGER, &[1, 0, 0])?;
    assert!(stinger.data.pointer("/payload/trigger_id").is_some());
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for record in records {
        assert!(schema.is_valid(&serde_json::to_value(record)?));
    }
    Ok(())
}

fn fixture() -> TestResult<Vec<u8>> {
    let items = [
        TestPlaylistItem::new(0, 1),
        TestPlaylistItem::new(SEGMENT, 0).play_mode(MODE).loops(-7),
    ];
    let mut bytes = TestBank::new(1)
        .object(TestHircObject::music_playlist_items(
            81,
            &[SEGMENT, SEGMENT],
            &items,
        ))
        .bytes();
    let flags = bytes.last_chunk_mut::<2>().ok_or("authored flags")?;
    *flags = [0x7f, 0x81];
    Ok(bytes)
}

fn check_item(item: &MediaRecord) {
    for (field, expected) in [
        ("segment_id", json!(SEGMENT)),
        ("item_id", json!(2)),
        ("play_mode_code", json!(MODE)),
        ("play_mode", json!("other")),
        ("loops", json!(-7)),
        ("uses_weight_code", json!(0x7f)),
        ("uses_weight", json!(true)),
        ("shuffle_code", json!(0x81)),
        ("shuffle", json!(true)),
    ] {
        assert_eq!(
            item.data.pointer(&format!("/payload/{field}")),
            Some(&expected),
            "authored field {field}"
        );
    }
}

fn check_flags(evidence: &Origins, row: usize, end: usize) -> TestResult {
    for (field, name, role, span) in [
        (
            "uses_weight_code",
            "musicPlaylistUsesWeight",
            "payload",
            end - 2..end - 1,
        ),
        (
            "uses_weight",
            "musicPlaylistUsesWeight",
            "conversion",
            end - 2..end - 1,
        ),
        (
            "shuffle_code",
            "musicPlaylistShuffle",
            "payload",
            end - 1..end,
        ),
        (
            "shuffle",
            "musicPlaylistShuffle",
            "conversion",
            end - 1..end,
        ),
    ] {
        let uses = evidence.uses(row, 1, &format!("/data/payload/{field}"))?;
        assert!(
            origins::has_exact(&uses, name, role, &span),
            "exact byte proof for {field}"
        );
    }
    Ok(())
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
        .ok_or_else(|| "original MusicPlaylist occurrence".into())
}
