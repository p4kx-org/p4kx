// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};

use p4kx_export::{CancelToken, ConflictPolicy, NDJSON_FILE, RecordExportOptions, RecordForm};
use p4kx_extraction::{ArchiveWriteProtection, ExtractionError, WriteProtectionError};

use crate::common::database;
use crate::record_export::export;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn an_ndjson_destination_that_is_the_source_is_not_replaced() -> TestResult {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("source.p4k");
    let out = dir.path().join(NDJSON_FILE);
    fs::write(&source, b"source")?;
    fs::hard_link(&source, &out)?;
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(&source, cancel.flag())?;
    let options = RecordExportOptions {
        protection: Some(&protection),
        cancel: Some(&cancel),
        policy: ConflictPolicy::Overwrite,
        ..RecordExportOptions::default()
    };
    let summary = export(
        &database().build(),
        RecordForm::Ndjson,
        dir.path(),
        &options,
    )?;
    assert!(matches!(
        summary.error(),
        Some(ExtractionError::Installation(
            WriteProtectionError::SourceOverwrite { .. }
        ))
    ));
    assert_eq!(summary.written(), 0);
    assert_eq!(fs::read(&source)?, b"source");
    assert_eq!(fs::read(&out)?, b"source");
    assert_eq!(fs::read_dir(dir.path())?.count(), 2);
    Ok(())
}

#[test]
fn an_ndjson_temporary_retarget_keeps_its_cleanup_failure_and_unpublished_counts() -> TestResult {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("source.p4k");
    fs::write(&source, b"source")?;
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(&source, cancel.flag())?;
    let changed = AtomicBool::new(false);
    let progress = || {
        if changed.swap(true, Ordering::Relaxed) {
            return;
        }
        let path = fs::read_dir(dir.path())
            .expect("directory")
            .map(|entry| entry.expect("entry").path())
            .find(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "part")
            })
            .expect("active temporary");
        fs::rename(&path, dir.path().join("retained.bin")).expect("move original");
        fs::hard_link(&source, &path).expect("unowned source alias");
    };
    let options = RecordExportOptions {
        protection: Some(&protection),
        cancel: Some(&cancel),
        progress: Some(&progress),
        ..RecordExportOptions::default()
    };
    let summary = export(
        &database().build(),
        RecordForm::Ndjson,
        dir.path(),
        &options,
    )?;
    assert!(matches!(
        summary.error(),
        Some(ExtractionError::CleanupFailed { .. })
    ));
    assert_eq!(summary.written(), 0);
    assert_eq!(summary.error_values(), 0);
    assert_eq!(fs::read(&source)?, b"source");
    assert!(!dir.path().join(NDJSON_FILE).exists());
    Ok(())
}
