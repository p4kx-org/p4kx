// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::time::{Duration, SystemTime};

use p4kx_extraction as extraction;
use p4kx_extraction::{
    ArchiveWatch, ArchiveWriteProtection, CancelToken, ConflictPolicy, ExtractOptions,
    ExtractionError,
};
use p4kx_testkit::TestEntry;

use super::entry_payload::{self as support, TestResult};

#[test]
fn archive_watch_stops_fixed_payload_before_reading_or_writing() -> TestResult {
    let fixture = support::fixture([TestEntry::stored("Data/", b"data")])?;
    let watch = ArchiveWatch::new(&fixture.archive);
    File::options()
        .write(true)
        .open(fixture.archive.path())?
        .set_modified(SystemTime::now() + Duration::from_secs(60))?;
    let report = extraction::extract_entry_payload(
        &fixture.archive,
        support::entry(&fixture, 0)?,
        &fixture.output,
        "payload",
        &ExtractOptions {
            watch: Some(&watch),
            ..ExtractOptions::default()
        },
    );
    assert!(report.archive_changed(), "the original watch is retained");
    assert_eq!(
        report.cancelled(),
        ["Data/"],
        "the physical source is reported as cancelled"
    );
    assert_eq!(
        fs::read_dir(&fixture.output)?.count(),
        0,
        "source change creates no output"
    );
    Ok(())
}

#[test]
fn fixed_payload_cannot_overwrite_its_source_and_preserves_the_original_refusal() -> TestResult {
    let fixture = support::fixture([TestEntry::stored("Data/", b"data")])?;
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(fixture.archive.path(), cancel.flag())?;
    let before = fs::read(fixture.archive.path())?;
    let report = extraction::extract_entry_payload(
        &fixture.archive,
        support::entry(&fixture, 0)?,
        fixture.folder.path(),
        "source.p4k",
        &ExtractOptions {
            policy: ConflictPolicy::Overwrite,
            protection: Some(&protection),
            cancel: Some(&cancel),
            ..ExtractOptions::default()
        },
    );
    assert!(
        matches!(
            report
                .failures()
                .first()
                .map(extraction::EntryFailure::error),
            Some(ExtractionError::Installation(_))
        ),
        "the shared source-protection refusal remains typed"
    );
    assert_eq!(
        fs::read(fixture.archive.path())?,
        before,
        "the source Archive is unchanged"
    );
    let names = fs::read_dir(fixture.folder.path())?
        .map(|item| item.map(|item| item.file_name()))
        .collect::<Result<Vec<_>, _>>()?;
    assert!(
        names
            .iter()
            .all(|name| !name.to_string_lossy().ends_with(".part")),
        "no partial file remains after refusal"
    );
    Ok(())
}

#[test]
fn conflict_skip_keeps_the_original_source_report_and_does_not_claim_verification() -> TestResult {
    let fixture = support::fixture([TestEntry::stored("Data/", b"data").with_crc32c(1)])?;
    fs::write(fixture.output.join("payload"), b"existing")?;
    let report = extraction::extract_entry_payload(
        &fixture.archive,
        support::entry(&fixture, 0)?,
        &fixture.output,
        "payload",
        &ExtractOptions {
            policy: ConflictPolicy::Skip,
            verify: true,
            ..ExtractOptions::default()
        },
    );
    assert_eq!(
        report.skipped(),
        ["Data/"],
        "conflict policy keeps original report identity"
    );
    assert_eq!(report.extracted(), 0, "a skipped payload is not written");
    assert!(
        report.failures().is_empty(),
        "skip has the same no-read/no-CRC contract as ordinary extraction"
    );
    assert_eq!(
        fs::read(fixture.output.join("payload"))?,
        b"existing",
        "skip preserves existing bytes"
    );
    Ok(())
}
