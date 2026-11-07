// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{
    Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken, ExtractOptions, ExtractionReport,
    Progress,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::checkpoint::Checkpoint;
use crate::staged_directory::StagedDirectory;
use crate::{Limits, MediaError, MediaWriter};

#[test]
fn cancellation_during_actual_extraction_retains_returned_counts()
-> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("payload.bin", vec![42; 8 << 20]))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let token = CancelToken::new();
    let watch = ArchiveWatch::new(&archive);
    let protection = ArchiveWriteProtection::new(archive.path(), token.flag())?;
    let checkpoint = Checkpoint {
        cancel: &token,
        watch: &watch,
        protection: &protection,
    };
    let staging = StagedDirectory::create(folder.path(), ".test-stage-", checkpoint)?;
    let writer = MediaWriter::new(
        staging.permit(),
        Vec::new(),
        Limits::default(),
        checkpoint,
        false,
    )?;
    let report = cancel_during_extraction(&archive, &token, &watch, &folder.path().join("entry"))?;
    assert!(
        token.is_cancelled(),
        "real byte progress must trigger cancellation"
    );
    let expected = (
        report.extracted(),
        report.failures().len(),
        report.cancelled().len(),
    );
    assert_eq!(expected, (0, 0, 1));
    let error = super::finish_extraction(report, &writer)
        .err()
        .ok_or("cancelled retention succeeded")?;
    let MediaError::ExtractionInterrupted { cause, report } = error else {
        return Err("interruption lost its original report".into());
    };
    assert!(matches!(*cause, MediaError::Cancelled));
    assert_eq!(
        (report.extracted, report.failed, report.cancelled),
        expected
    );
    assert!(!folder.path().join("media-index.json").exists());
    staging.close()?;
    Ok(())
}

fn cancel_during_extraction(
    archive: &Archive,
    token: &CancelToken,
    watch: &ArchiveWatch,
    output: &std::path::Path,
) -> Result<ExtractionReport, Box<dyn std::error::Error>> {
    fs::create_dir(output)?;
    let entry = archive.entries().first().ok_or("physical Entry")?;
    let cancel_during_copy = |progress: Progress| {
        if progress.bytes_done > 0 && progress.bytes_done < progress.bytes_total {
            token.cancel();
        }
    };
    Ok(p4kx_extraction::extract(
        archive,
        &[entry],
        output,
        &ExtractOptions {
            verify: true,
            cancel: Some(token),
            watch: Some(watch),
            progress: Some(&cancel_during_copy),
            ..ExtractOptions::default()
        },
    ))
}
