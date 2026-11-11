// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{Archive, OpenOptions, Wem};
use p4kx_media::{Artifact, AudioOutput, ExportOptions, Status};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestCodec, TestEntry, TestWem, test_key};
use serde_json::json;

use super::origins::Origins;
use super::{AUDIO_STREAM, TestResult, check_schema, options};
use crate::support;

const BANK: &str = "samples.bnk";
const NAMESPACE: &str = "audio-bank-wem-conversion";
const CONSUMED: u64 = 0x48 + 32;

#[test]
fn requested_adpcm_wav_retains_exact_embedded_coded_blocks_and_encryption() -> TestResult {
    let folder = tempfile::tempdir()?;
    let coded = TestWem::encoded(TestCodec::Tag(0x0002), 2, 48_000, 0)
        .block_align(0x48)
        .data(vec![0; usize::try_from(CONSUMED)? + 1])
        .smpl_loop(0, 1, 2)
        .bytes();
    let bank = TestBank::new(7).wem(49, coded.clone()).bytes();
    let bank_offset = bank
        .windows(4)
        .position(|chunk| chunk == b"RIFF")
        .ok_or("embedded RIFF header")?;
    let original = Wem::parse(coded.as_slice())?;
    let start = original.metadata().data_chunk().body_offset;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("notes.txt", b"fixture".as_slice()))
        .entry(TestEntry::zstd(BANK, bank.as_slice()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let out = folder.path().join("out");
    let index = support::published(p4kx_media::export(
        &archive,
        &out,
        &ExportOptions {
            audio_output: AudioOutput::Wav,
            ..options()
        },
    )?)?;
    let rows = support::records(&out, &index, AUDIO_STREAM)?;
    check_schema(&rows)?;
    let (ordinal, row) = rows
        .iter()
        .enumerate()
        .find(|(_, row)| row.subresource.namespace == NAMESPACE)
        .ok_or("embedded ADPCM conversion record")?;
    assert_eq!(
        row.status,
        Status::Complete,
        "the supported ADPCM layout converts"
    );
    assert_eq!(row.subresource.indices, [1, 0]);
    assert_eq!(
        row.data.pointer("/payload/requested_format"),
        Some(&json!("wav"))
    );
    assert_eq!(
        row.data.pointer("/payload/source_codec"),
        Some(&json!("ima_adpcm"))
    );
    let payload = row
        .data
        .pointer("/payload/outcome/payload")
        .ok_or("written conversion")?;
    let artifact: Artifact =
        serde_json::from_value(payload.get("artifact").ok_or("artifact mapping")?.clone())?;
    let base = out.join(&index.artifact_base);
    check_artifact(&base, &artifact, &original)?;
    check_range(payload, u64::try_from(bank_offset)? + start);
    check_origin(&Origins::read(&base)?, ordinal, start)?;
    Ok(())
}

fn check_artifact(
    base: &std::path::Path,
    artifact: &Artifact,
    original: &Wem<&[u8]>,
) -> TestResult {
    let mut legacy = Vec::new();
    let report = original.write_wav(&mut legacy)?;
    assert_eq!(
        report.sample_count, 88,
        "known original block and partial IMA sub-blocks"
    );
    assert_eq!(
        fs::read(base.join(&artifact.path))?,
        legacy,
        "canonical original decoder output"
    );
    Ok(())
}

fn check_range(payload: &serde_json::Value, start: u64) {
    assert_eq!(
        payload.get("actual_codec"),
        Some(&json!("pcm")),
        "WAV contains decoded PCM"
    );
    assert_eq!(
        payload.get("actual_format"),
        Some(&json!("wav")),
        "the requested WAV artifact is produced"
    );
    assert_eq!(
        payload.get("sample_count"),
        Some(&json!("88")),
        "the usable IMA tail contributes frames"
    );
    assert_eq!(
        payload.get("source_range"),
        Some(&json!({
            "basis":{"type":"entry"}, "start":start, "end_exclusive":start + CONSUMED
        })),
        "the consumed source range excludes bytes outside complete and usable blocks"
    );
}

fn check_origin(origins: &Origins, ordinal: usize, start: u64) -> TestResult {
    let uses = origins.for_field(
        ordinal,
        1,
        "/data/payload/outcome/payload/artifact/digest/hex",
    )?;
    let expected = json!({
        "type":"subresource", "namespace":"wemAdpcmBlocks", "field":"originalBlocks",
        "indices":["1","0", &start.to_string(), &(start + CONSUMED).to_string()]
    });
    assert!(
        uses.iter()
            .any(|source| source.get("location") == Some(&expected)
                && source.get("role") == Some(&json!("conversion"))),
        "only the admitted coded bytes are an artifact input"
    );
    Ok(())
}
