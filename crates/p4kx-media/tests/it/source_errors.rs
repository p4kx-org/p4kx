// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{Archive, ExtractionError, archive_error_in};
use p4kx_media::{ExportOptions, Identity, MediaError};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::support::TestResult;

#[test]
fn missing_and_wrong_key_causes_remain_typed_behind_exact_source_context() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("secret.bin", b"secret").encrypted())
        .write_to(&path)?;
    for (name, key) in [
        ("missing", None),
        ("wrong", Some(p4kx_testkit::other_test_key())),
    ] {
        let archive = Archive::open_with_key(&path, key)?;
        let result = p4kx_media::export(
            &archive,
            &folder.path().join(name),
            &ExportOptions::default(),
        );
        let error = result
            .err()
            .ok_or("encrypted source unexpectedly succeeded")?;
        let cause = archive_error_in(&error).ok_or("original Archive error disappeared")?;
        assert!(
            if name == "missing" {
                cause.is_key_required()
            } else {
                cause.is_wrong_key()
            },
            "wrong typed key diagnosis for {name}"
        );
        let MediaError::Source { entry, cause } = &error else {
            return Err("missing physical source context".into());
        };
        assert!(
            matches!(&entry.identity, Identity::Entry { entry, containers } if entry.entry_index == 0 && containers.is_empty())
        );
        let MediaError::Extraction { report, .. } = cause.as_ref() else {
            return Err("missing original extraction report".into());
        };
        assert_eq!(
            (report.extracted, report.failed, report.cancelled),
            (0, 1, 0)
        );
    }
    Ok(())
}

#[test]
fn duplicate_name_failure_retains_the_second_physical_index_and_checksum_error() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("same.bin", b"valid"))
        .entry(TestEntry::stored("same.bin", b"invalid").with_crc32c(1))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let error = p4kx_media::export(
        &archive,
        &folder.path().join("media"),
        &ExportOptions::default(),
    )
    .err()
    .ok_or("corrupt duplicate succeeded")?;
    let MediaError::Source { entry, cause } = error else {
        return Err("physical failure identity".into());
    };
    assert!(
        matches!(entry.identity, Identity::Entry { entry, .. } if entry.entry_index == 1),
        "first duplicate must not substitute for failed second Entry"
    );
    let MediaError::Extraction { cause, report } = *cause else {
        return Err("extraction failure".into());
    };
    assert!(
        matches!(*cause, ExtractionError::ChecksumMismatch { .. }),
        "original checksum mismatch survives"
    );
    assert_eq!((report.extracted, report.failed), (0, 1));
    Ok(())
}

#[test]
fn corrupt_local_header_preserves_archive_format_error_and_decoder_fault_preserves_io() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("header.p4k");
    let mut bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("header.bin", b"data"))
        .build();
    *bytes.first_mut().ok_or("local signature")? = 0;
    fs::write(&path, bytes)?;
    let archive = Archive::open(path)?;
    let error = p4kx_media::export(
        &archive,
        &folder.path().join("header"),
        &ExportOptions::default(),
    )
    .err()
    .ok_or("corrupt header succeeded")?;
    assert!(
        archive_error_in(&error).is_some(),
        "original format error survives Media and Extraction wrappers"
    );
    let path = folder.path().join("decoder.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("decoder.bin", b"payload").with_payload([0x28, 0xb5, 0x2f, 0xfd]))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let error = p4kx_media::export(
        &archive,
        &folder.path().join("decoder"),
        &ExportOptions::default(),
    )
    .err()
    .ok_or("truncated decoder succeeded")?;
    let MediaError::Source { cause, .. } = error else {
        return Err("decoder physical context".into());
    };
    let MediaError::Extraction { cause, .. } = *cause else {
        return Err("decoder extraction error".into());
    };
    assert!(
        matches!(*cause, ExtractionError::Read(_)),
        "actual decoder I/O error stays typed"
    );
    Ok(())
}
