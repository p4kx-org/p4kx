// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, TestHircObject, test_key};
use serde_json::json;

use super::audio_bank_hirc_action_origins as origins;
use super::audio_bank_state_provider::source_span;
use super::support;
use super::support::audio_provider::{Origins, Owner, TestResult};

const MUSIC: [&str; 4] = [
    "audio-bank-music-segment",
    "audio-bank-music-playlist",
    "audio-bank-music-switch",
    "audio-bank-music-track",
];

#[test]
fn music_objects_reuse_source_bound_shared_node_groups() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = TestBank::new(7)
        .object(TestHircObject::music_segment(11, &[17], &["Entry"]))
        .object(TestHircObject::music_playlist(12, &[11]))
        .object(TestHircObject::music_switch(13, &[11], 99, &[11]))
        .object(TestHircObject::music_track(17, &[24]))
        .bytes();
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("music.bnk", source.as_slice()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let output = folder.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::Audio],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "audio.jsonl")?;
    let receipts = Origins::read(&output.join(&index.artifact_base))?;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in &rows {
        assert!(
            schema.is_valid(&serde_json::to_value(row)?),
            "music row validates"
        );
    }
    verify_groups(&source, &rows, &receipts)?;
    let (_, property) = row(&rows, "audio-bank-hirc-node-property", &[1, 0, 0, 0])?;
    assert_eq!(
        property.data.pointer("/payload/words/scalar_bits/value"),
        Some(&json!(3.0_f32.to_bits()))
    );
    let (_, point) = row(
        &rows,
        "audio-bank-hirc-music-node-rtpc-point",
        &[1, 0, 0, 0],
    )?;
    assert!(
        point.data.pointer("/payload/from_bits/value").is_some(),
        "populated music RTPC point"
    );
    Ok(())
}

fn verify_groups(source: &[u8], rows: &[MediaRecord], receipts: &Origins) -> TestResult {
    for (object, namespace) in MUSIC.into_iter().enumerate() {
        let owner = [1, u64::try_from(object)?];
        let (_, original) = row(rows, namespace, &owner)?;
        assert_eq!(
            original.status,
            Status::Complete,
            "existing music owner is decoded"
        );
        let (ordinal, properties) = row(rows, "audio-bank-hirc-node-properties", &owner)?;
        verify_properties(source, receipts, ordinal, properties)?;
        for group in [
            "audio-bank-hirc-node-aux",
            "audio-bank-hirc-node-positioning",
            "audio-bank-hirc-node-advanced-settings",
            "audio-bank-hirc-inline-state",
            "audio-bank-hirc-music-node-rtpc",
        ] {
            let (_, row) = row(rows, group, &owner)?;
            assert_eq!(
                row.data.pointer("/payload/layout"),
                Some(&json!("decoded")),
                "checked shared layout"
            );
        }
    }
    Ok(())
}

fn verify_properties(
    source: &[u8],
    receipts: &Origins,
    ordinal: usize,
    row: &MediaRecord,
) -> TestResult {
    let field = "/payload/observed/priority_flags";
    assert_eq!(
        row.data.pointer(&format!("{field}/value")),
        Some(&json!(0)),
        "explicit zero"
    );
    let bytes = source_span(source, &row.data, &format!("{field}/range"))?;
    assert_eq!(bytes, &[0], "one original flag byte");
    let datum = receipts.field("records", ordinal, &format!("/data{field}/value"), true)?;
    let uses = receipts.require_owner(
        datum,
        Owner {
            path: "music.bnk",
            entry: 0,
            parent: None,
            stored: "encrypted",
            metadata: false,
            payload: true,
        },
    )?;
    assert!(
        origins::has_field(&uses, "nodePriorityFlags"),
        "same owning bank receipt"
    );
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
        .ok_or_else(|| format!("missing {namespace} at synthetic position {indices:?}").into())
}
