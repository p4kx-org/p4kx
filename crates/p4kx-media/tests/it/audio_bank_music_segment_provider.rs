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

#[test]
fn music_segment_export_keeps_grid_stinger_and_marker_source_ranges() -> TestResult {
    let folder = tempfile::tempdir()?;
    let mut bank = TestBank::new(1)
        .object(TestHircObject::music_segment(
            71,
            &[21, 21],
            &["Entry", "Exit"],
        ))
        .bytes();
    let start = bank
        .windows(5)
        .position(|window| window == b"Exit\0")
        .ok_or("fixture marker")?;
    *bank.get_mut(start).ok_or("original marker byte")? = 0xff;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("notes.txt", b"fixture".as_slice()))
        .entry(TestEntry::zstd("music.bnk", bank.as_slice()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let options = ExportOptions {
        kinds: vec![MediaKind::Audio],
        ..ExportOptions::default()
    };
    let out = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &out, &options)?)?;
    let base = out.join(&index.artifact_base);
    let rows = support::records(&out, &index, "audio.jsonl")?;
    let (ordinal, marker) = row(&rows, "audio-bank-music-segment-marker", &[1, 0, 1])?;
    assert_eq!(
        marker.status,
        Status::Complete,
        "opaque marker encoding does not erase the original row"
    );
    assert_eq!(
        marker.data.pointer("/payload/encoding"),
        Some(&json!("invalid-utf8")),
        "invalid UTF-8 is labelled explicitly"
    );
    assert_eq!(
        marker.data.pointer("/payload/name"),
        Some(&json!("\u{fffd}xit")),
        "the display name uses the replacement character"
    );
    let evidence = Origins::read(&base)?;
    let names = evidence.uses(ordinal, 1, "/data/payload/name")?;
    assert!(
        origins::has_exact(&names, "musicMarkerName", "conversion", &(start..start + 4)),
        "the name receipt selects only the original four bytes"
    );
    let raw = marker.source.raw.as_ref().ok_or("retained original bank")?;
    assert_eq!(
        fs::read(base.join(&raw.path))?,
        bank,
        "the original invalid byte survives in the retained bank"
    );
    check_structure(&rows)?;
    Ok(())
}

fn check_structure(rows: &[MediaRecord]) -> TestResult {
    let (_, segment) = row(rows, "audio-bank-music-segment", &[1, 0])?;
    assert_eq!(
        segment.data.pointer("/payload/child_count"),
        Some(&json!(2)),
        "duplicate child occurrences contribute to the authored count"
    );
    assert!(
        segment
            .data
            .pointer("/payload/grid_period/raw_bits")
            .is_some(),
        "the original period word remains available"
    );
    assert!(
        segment
            .data
            .pointer("/payload/grid_offset/raw_bits")
            .is_some(),
        "the original offset word remains available"
    );
    let (_, stinger) = row(rows, "audio-bank-music-segment-stinger", &[1, 0, 0])?;
    assert!(
        stinger.data.pointer("/payload/trigger_id").is_some(),
        "the stinger retains its authored trigger ID"
    );
    assert!(
        stinger.data.pointer("/payload/segment_id").is_some(),
        "the stinger retains its authored segment ID"
    );
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for record in rows {
        assert!(
            validator.is_valid(&serde_json::to_value(record)?),
            "each emitted occurrence satisfies the built-in record schema"
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
        .ok_or_else(|| "original MusicSegment occurrence".into())
}
