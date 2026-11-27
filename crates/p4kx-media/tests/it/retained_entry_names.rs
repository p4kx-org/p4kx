// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{Archive, Checksum, ExtractionError};
use p4kx_media::{ExportOptions, Identity, MediaError, MediaKind};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use sha2::{Digest as _, Sha256};

use super::support::{self, TestResult};

#[test]
fn directory_and_invalid_utf8_named_entries_keep_physical_records_and_exact_payloads() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("source.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/", b""))
        .entry(TestEntry::stored("x", b"opaque bytes").with_path_bytes(b"Data/\xff/"))
        .entry(TestEntry::zstd("normal.bin", b"normal bytes"))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let output = folder.path().join("media");
    let options = ExportOptions {
        kinds: vec![MediaKind::Archive],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "archive.jsonl")?;
    let physical = rows
        .iter()
        .filter(|row| row.subresource.namespace == "archive-entry")
        .collect::<Vec<_>>();
    assert_eq!(
        physical.len(),
        3,
        "directory-named physical records are never blanket skipped"
    );
    let expected: [&[u8]; 3] = [b"", b"opaque bytes", b"normal bytes"];
    let entries = physical.into_iter().zip(expected).enumerate();
    for (position, (row, expected)) in entries {
        let Identity::Entry { containers, entry } = &row.source.identity else {
            return Err("physical Entry identity".into());
        };
        assert!(containers.is_empty(), "root source stays root-local");
        assert_eq!(
            entry.entry_index, position,
            "physical ordinals remain original"
        );
        let raw = row.source.raw.as_ref().ok_or("retained raw reference")?;
        assert_eq!(
            fs::read(output.join(&index.artifact_base).join(&raw.path))?,
            expected,
            "retained decoded bytes are exact"
        );
        assert_eq!(
            raw.digest.hex,
            Checksum::Sha256(Sha256::digest(expected).into()).hex(),
            "digest covers all payload bytes, including empty payloads"
        );
    }
    assert_eq!(
        index
            .kinds
            .first()
            .ok_or("archive counts")?
            .counts
            .source_entries
            .written,
        3,
        "actual sources remain counted separately from directory metadata rows"
    );
    Ok(())
}

#[test]
fn a_directory_named_zero_payload_is_really_verified_before_publication() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("source.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/", b"").with_crc32c(1))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let output = folder.path().join("media");
    let error = p4kx_media::export(&archive, &output, &ExportOptions::default())
        .err()
        .ok_or("mismatching empty payload succeeded")?;
    let MediaError::Source { entry, cause } = error else {
        return Err("physical source cause".into());
    };
    assert!(
        matches!(entry.identity, Identity::Entry { entry, .. } if entry.entry_index == 0),
        "physical source identity is retained"
    );
    let MediaError::Extraction { cause, report } = *cause else {
        return Err("original extraction report".into());
    };
    assert!(
        matches!(*cause, ExtractionError::ChecksumMismatch { .. }),
        "verification failure is not masked by source filename failure"
    );
    assert_eq!(
        (report.extracted, report.failed, report.cancelled),
        (0, 1, 0),
        "actual kernel outcome counts survive"
    );
    assert!(
        !output.join("media-index.json").exists(),
        "unverified payload cannot publish a generation"
    );
    Ok(())
}
