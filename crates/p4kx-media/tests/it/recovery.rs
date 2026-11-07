// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::support::TestResult;

#[test]
fn a_recovered_inventory_never_becomes_a_complete_root_manifest() -> TestResult {
    const ZIP_END_RECORD_BYTES: usize = 22;
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data.p4k");
    let mut bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("known.txt", b"known"))
        .build();
    bytes.truncate(bytes.len().saturating_sub(ZIP_END_RECORD_BYTES));
    fs::write(&source, bytes)?;
    let archive = Archive::open_with(source, OpenOptions::new().recover(true))?;
    let output = folder.path().join("media");
    assert!(
        matches!(
            p4kx_media::export(&archive, &output, &ExportOptions::default()),
            Err(p4kx_media::MediaError::IncompleteInventory { .. })
        ),
        "normal export refuses a proven incomplete inventory before retention"
    );
    assert!(
        !output.exists(),
        "normal inventory refusal performs no output work"
    );
    let options = ExportOptions {
        diagnostic: true,
        ..ExportOptions::default()
    };
    let result = p4kx_media::export(&archive, &output, &options)?;
    let ExportOutcome::Incomplete { index, diagnostic } = result else {
        return Err("recovered directory claimed complete generation".into());
    };
    assert!(
        !index.enumeration_complete,
        "source inventory is a lower bound after recovery"
    );
    assert!(!index.source.enumeration_complete);
    assert!(
        index
            .source
            .directory_issues
            .iter()
            .any(|issue| issue.code == "p4kx.archive.recovered-directory")
    );
    assert!(diagnostic.is_file());
    assert!(
        !output.join("media-index.json").exists(),
        "recovered inventory has no complete commit point"
    );
    Ok(())
}
