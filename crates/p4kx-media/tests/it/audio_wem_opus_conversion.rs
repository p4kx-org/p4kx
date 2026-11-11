// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::sound::{WemOpusRanges, WemSource};
use p4kx_extraction::{Archive, OpenOptions, Wem};
use p4kx_media::{Artifact, AudioOutput, ExportOptions, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, TestOpus, TestOpusFraming, test_key};
use serde_json::{Value, json};

use super::origins::Origins;
use super::{AUDIO_STREAM, TestResult, check_schema, options};
use crate::support;

#[test]
fn requested_opus_ogg_keeps_the_checked_and_emitted_ranges_of_an_encrypted_bank_wem() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let bytes = TestOpus::new(TestOpusFraming::Wwise, 2, 9_600)
        .declared_sample_count(960)
        .wem()?
        .smpl_loop(0, 1, 2)
        .bytes();
    let bank = TestBank::new(7).wem(49, bytes.clone()).bytes();
    let bank_offset = bank
        .windows(4)
        .position(|chunk| chunk == b"RIFF")
        .ok_or("embedded WEM")? as u64;
    let archive = encrypted_bank(folder.path(), &bank)?;
    let out = folder.path().join("out");
    let index = support::published(p4kx_media::export(
        &archive,
        &out,
        &ExportOptions {
            audio_output: AudioOutput::Ogg,
            ..options()
        },
    )?)?;
    let records = support::records(&out, &index, AUDIO_STREAM)?;
    check_schema(&records)?;
    let (ordinal, row) = records
        .iter()
        .enumerate()
        .find(|(_, row)| row.subresource.namespace == "audio-bank-wem-conversion")
        .ok_or("embedded Ogg conversion row")?;
    let payload = check_request(row)?;
    let base = out.join(&index.artifact_base);
    check_artifact(&base, payload, &bytes)?;
    let mut source = WemSource::open(std::io::Cursor::new(&bytes), bytes.len() as u64)?;
    let mut inspected = source.inspect()?;
    let prepared = inspected.opus_ogg()?;
    let ranges = prepared.ranges();
    check_ranges(payload, bank_offset, ranges)?;
    check_origin(&Origins::read(&base)?, ordinal, ranges)?;
    Ok(())
}

fn check_request(row: &MediaRecord) -> TestResult<&Value> {
    assert_eq!(
        row.status,
        Status::Complete,
        "the supported Opus source converts"
    );
    assert_eq!(row.subresource.indices, [1, 0], "actual embedded WEM");
    assert_eq!(
        row.data.pointer("/payload/requested_format"),
        Some(&json!("ogg")),
        "requested container"
    );
    assert_eq!(
        row.data.pointer("/payload/source_codec"),
        Some(&json!("opus")),
        "original codec"
    );
    let payload = row
        .data
        .pointer("/payload/outcome/payload")
        .ok_or("written artifact mapping")?;
    assert_eq!(
        payload.get("actual_format"),
        Some(&json!("ogg")),
        "actual container"
    );
    assert_eq!(
        payload.get("actual_codec"),
        Some(&json!("opus")),
        "actual output codec"
    );
    let mut wrong_container = serde_json::to_value(row)?;
    *wrong_container
        .pointer_mut("/data/payload/outcome/payload/actual_format")
        .ok_or("written container field")? = json!("wav");
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    assert!(
        !schema.is_valid(&wrong_container),
        "Opus receipt requires an Ogg container"
    );
    Ok(payload)
}

fn check_artifact(base: &Path, payload: &Value, bytes: &[u8]) -> TestResult {
    let artifact: Artifact =
        serde_json::from_value(payload.get("artifact").ok_or("verified artifact")?.clone())?;
    let original = Wem::parse(bytes)?;
    let mut expected = Vec::new();
    let report = original.write_ogg(&mut expected)?;
    assert_eq!(
        fs::read(base.join(&artifact.path))?,
        expected,
        "deterministic Opus pages"
    );
    assert_eq!(
        payload.get("sample_count"),
        Some(&json!(report.sample_count.to_string())),
        "actual authored trim after pre-skip"
    );
    Ok(())
}

fn check_ranges(payload: &Value, offset: u64, ranges: &WemOpusRanges) -> TestResult {
    let seek = ranges
        .seek_table
        .as_ref()
        .ok_or("original Wwise seek words")?;
    for (field, span) in [
        ("/source_range", &ranges.emitted_data),
        ("/opus_inputs/format", &ranges.format),
        ("/opus_inputs/seek_table", seek),
        ("/opus_inputs/checked_data", &ranges.checked_data),
    ] {
        assert_eq!(
            payload.pointer(field),
            Some(&json!({
                "basis":{"type":"entry"},
                "start":offset + span.start,
                "end_exclusive":offset + span.end
            })),
            "{field} has exact enclosing-bank coordinates"
        );
    }
    assert!(
        ranges.emitted_data.end < ranges.checked_data.end,
        "later checked packets are trimmed"
    );
    assert_eq!(
        payload.pointer("/opus_inputs/embedded_headers"),
        Some(&Value::Null),
        "table framing"
    );
    Ok(())
}

fn check_origin(origins: &Origins, ordinal: usize, ranges: &WemOpusRanges) -> TestResult {
    let uses = origins.for_field(
        ordinal,
        1,
        "/data/payload/outcome/payload/artifact/digest/hex",
    )?;
    let seek = ranges
        .seek_table
        .as_ref()
        .ok_or("original table for output proof")?;
    for (field, span) in [
        ("seekTable", seek),
        ("checkedPackets", &ranges.checked_data),
        ("emittedPackets", &ranges.emitted_data),
    ] {
        let expected = json!({
            "type":"subresource", "namespace":"wemOpusInput", "field":field,
            "indices":["1","0", span.start.to_string(), span.end.to_string()]
        });
        assert!(
            uses.iter().any(|source| {
                source.get("location") == Some(&expected)
                    && source.get("role") == Some(&json!("conversion"))
            }),
            "{field} has its original encrypted bank source"
        );
    }
    Ok(())
}

fn encrypted_bank(directory: &Path, bytes: &[u8]) -> TestResult<Archive> {
    let path = directory.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("notes.txt", b"fixture".as_slice()))
        .entry(TestEntry::zstd("opus.bnk", bytes).encrypted())
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}
