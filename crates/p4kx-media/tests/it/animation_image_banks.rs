// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, Filter, OpenOptions};
use p4kx_media::{ExportOptions, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestAnimation, TestChunkFile, TestDba, TestEntry, test_key};
use serde_json::Value;

use super::{CAF_CHUNK, IMAGE, PATH_BYTES, TestResult, field, image, options, support};

const BANK: &str = "Data/Animations/Fixture/bank.dba";
const INDEXED_PATH: &str = "Animations/Fixture/Exact.caf";

#[test]
fn image_candidates_require_original_bank_and_exact_metadata_path() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive_path = scratch.path().join("fixture.p4k");
    let mut source = image(&[field(INDEXED_PATH.as_bytes())])?;
    source
        .get_mut(8 + PATH_BYTES + 4..8 + PATH_BYTES + 8)
        .ok_or("DBA hash word")?
        .copy_from_slice(&crc32fast::hash(b"animations/fixture/bank.dba").to_le_bytes());
    let file = TestChunkFile::ivo().chunk(CAF_CHUNK, 0x902, source).bytes();
    let bank = TestDba::new()
        .animation(TestAnimation::new("animations/different/exact.caf", 30))
        .animation(TestAnimation::new("animations/fixture/exact.caf", 30))
        .pool_after_metadata()
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(IMAGE, file))
        .entry(TestEntry::stored(BANK, bank).encrypted())
        .write_to(&archive_path)?;
    let archive = Archive::open_with(archive_path, OpenOptions::new().key(Some(test_key())))?;
    let output = scratch.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let rows = support::records(&output, &index, "animation.jsonl")?;
    let row = image_record(&rows)?;
    check_match(row);
    check_origins(&output.join(&index.artifact_base), &rows)?;
    check_filtered(&archive, scratch.path())
}

fn check_match(row: &MediaRecord) {
    assert_eq!(
        text(row, "/payload/dba_entry/resolution/status"),
        Some("resolved"),
        "hash finds physical bank"
    );
    assert_eq!(
        text(row, "/payload/dba_clip/status"),
        Some("observed"),
        "actual metadata was read"
    );
    assert_eq!(
        row.data
            .pointer("/payload/dba_clip/observed_candidates")
            .and_then(Value::as_u64),
        Some(1),
        "same basename in other folders is ignored"
    );
    assert_eq!(
        row.data
            .pointer("/payload/dba_clip/candidates/0/clip_ordinal")
            .and_then(Value::as_u64),
        Some(1),
        "physical metadata ordinal retained"
    );
}

fn check_filtered(archive: &Archive, scratch: &Path) -> TestResult {
    let filtered = ExportOptions {
        include: Some(Filter::parse(IMAGE)?),
        ..options()
    };
    let output = scratch.join("filtered");
    let index = support::published(p4kx_media::export(archive, &output, &filtered)?)?;
    let rows = support::records(&output, &index, "animation.jsonl")?;
    let row = image_record(&rows)?;
    assert_eq!(
        text(row, "/payload/dba_entry/resolution/status"),
        Some("not-selected"),
        "filtered bank directory receipt survives"
    );
    assert_eq!(
        text(row, "/payload/dba_clip/status"),
        Some("bank-unresolved"),
        "filtered bank contributes no payload"
    );
    Ok(())
}

fn image_record(rows: &[MediaRecord]) -> TestResult<&MediaRecord> {
    rows.iter()
        .find(|row| row.subresource.namespace == "animation-image-record")
        .ok_or_else(|| "global image record".into())
}

fn text<'a>(row: &'a MediaRecord, pointer: &str) -> Option<&'a str> {
    row.data.pointer(pointer).and_then(Value::as_str)
}

fn check_origins(base: &Path, rows: &[MediaRecord]) -> TestResult {
    let ordinal = rows
        .iter()
        .position(|row| row.subresource.namespace == "animation-image-record")
        .ok_or("CAF image source row")?
        .to_string();
    let data = fs::read_to_string(base.join("field-origins.jsonl"))?;
    let mut verified = 0;
    for line in data.lines() {
        let row: Value = serde_json::from_str(line)?;
        if row.pointer("/scope/ordinal").and_then(Value::as_str) != Some(ordinal.as_str())
            || row.pointer("/scope/stream/kind").and_then(Value::as_str) != Some("animation")
        {
            continue;
        }
        let expected = match row.get("field").and_then(Value::as_str) {
            Some("/data/payload/dba_path_crc32") => false,
            Some("/data/payload/dba_clip/candidates/0/clip_ordinal") => true,
            _ => continue,
        };
        assert_eq!(
            row.get("containsEncryptedSource").and_then(Value::as_bool),
            Some(expected),
            "plain index word and encrypted DBA metadata retain separate evidence",
        );
        verified += 1;
    }
    assert_eq!(verified, 2, "both immediate receipts are present");
    Ok(())
}
