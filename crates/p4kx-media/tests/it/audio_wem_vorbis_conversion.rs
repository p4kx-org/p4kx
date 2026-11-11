// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{Archive, OpenOptions, Wem};
use p4kx_media::{Artifact, AudioOutput, ExportOptions, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, TestVorbis, test_key};
use serde_json::{Value, json};

use super::origins::Origins;
use super::{AUDIO_STREAM, TestResult, check_schema, options};
use crate::support;

#[test]
fn requested_vorbis_ogg_retains_exact_embedded_packet_ranges_and_encryption() -> TestResult {
    let folder = tempfile::tempdir()?;
    let bytes = TestVorbis::new(2, 48_000, &[false, true, false, false])
        .wem()
        .smpl_loop(0, 1, 2)
        .tail_chunk(*b"data", [0x70, 0x74])
        .bytes();
    let bank = TestBank::new(7).wem(49, bytes.clone()).bytes();
    let bank_offset = bank
        .windows(4)
        .position(|chunk| chunk == b"RIFF")
        .ok_or("embedded RIFF header")? as u64;
    let original = Wem::parse(bytes.as_slice())?;
    let start = original.metadata().data_chunk().body_offset;
    let setup = original.data().first_chunk::<2>().ok_or("setup prefix")?;
    let audio_start = start + 2 + u64::from(u16::from_le_bytes(*setup));
    let end = original.metadata().data_chunk().body_end_exclusive();
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
        .ok_or("embedded Ogg conversion record")?;
    check_request(row);
    let payload = row
        .data
        .pointer("/payload/outcome/payload")
        .ok_or("written Ogg")?;
    let artifact: Artifact = serde_json::from_value(
        payload
            .get("artifact")
            .ok_or("verified Ogg artifact")?
            .clone(),
    )?;
    let base = out.join(&index.artifact_base);
    check_artifact(&base, &artifact, &original, payload)?;
    let fmt = original.metadata().info_source().format_body_offset();
    check_payload(payload, bank_offset, start, audio_start, end, fmt);
    check_origin(&Origins::read(&base)?, ordinal, start, audio_start, end)?;
    Ok(())
}

fn check_artifact(
    base: &std::path::Path,
    artifact: &Artifact,
    original: &Wem<&[u8]>,
    payload: &Value,
) -> TestResult {
    let mut expected = Vec::new();
    let report = original.write_ogg(&mut expected)?;
    assert_eq!(
        fs::read(base.join(&artifact.path))?,
        expected,
        "deterministic Ogg pages"
    );
    assert_eq!(
        payload.get("sample_count"),
        Some(&json!(report.sample_count.to_string())),
        "actual complete output frame count"
    );
    Ok(())
}

fn check_request(row: &MediaRecord) {
    assert_eq!(row.status, Status::Complete, "Vorbis conversion succeeds");
    assert_eq!(row.subresource.indices, [1, 0], "original embedded WEM");
    assert_eq!(
        row.data.pointer("/payload/requested_format"),
        Some(&json!("ogg")),
        "requested Ogg container"
    );
    assert_eq!(
        row.data.pointer("/payload/source_codec"),
        Some(&json!("vorbis")),
        "source codec"
    );
}

fn check_payload(
    payload: &Value,
    bank_offset: u64,
    start: u64,
    audio_start: u64,
    end: u64,
    fmt: u64,
) {
    assert_eq!(
        payload.get("actual_codec"),
        Some(&json!("vorbis")),
        "actual Ogg codec"
    );
    assert_eq!(
        payload.get("actual_format"),
        Some(&json!("ogg")),
        "actual output container"
    );
    for (path, start, end) in [
        ("/source_range", audio_start, end),
        ("/vorbis_inputs/setup_packet", start, audio_start),
        ("/vorbis_inputs/average_byte_rate", fmt + 8, fmt + 12),
        ("/vorbis_inputs/header", fmt + 24, fmt + 66),
    ] {
        assert_eq!(
            payload.pointer(path),
            Some(&json!({
                "basis":{"type":"entry"},
                "start":bank_offset + start,
                "end_exclusive":bank_offset + end
            })),
            "{path} has the exact enclosing bank coordinates"
        );
    }
}

fn check_origin(
    origins: &Origins,
    ordinal: usize,
    start: u64,
    audio_start: u64,
    end: u64,
) -> TestResult {
    let uses = origins.for_field(
        ordinal,
        1,
        "/data/payload/outcome/payload/artifact/digest/hex",
    )?;
    for (field, start, end) in [
        ("setupPacket", start, audio_start),
        ("audioPackets", audio_start, end),
    ] {
        let expected = json!({
            "type":"subresource", "namespace":"wemVorbisInput", "field":field,
            "indices":["1","0", &start.to_string(), &end.to_string()]
        });
        assert!(
            uses.iter().any(|source| {
                source.get("location") == Some(&expected)
                    && source.get("role") == Some(&json!("conversion"))
            }),
            "{field} is an exact encrypted artifact input"
        );
    }
    Ok(())
}

fn encrypted_bank(directory: &std::path::Path, bank: &[u8]) -> TestResult<Archive> {
    let path = directory.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("notes.txt", b"fixture".as_slice()))
        .entry(TestEntry::zstd("samples.bnk", bank).encrypted())
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}
