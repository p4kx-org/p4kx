// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestContainer, TestEntry};

use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::SourceDisposition;

#[test]
fn a_failed_soc_does_not_hide_later_selected_sources() -> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let container = TestContainer::new()
        .entry(TestEntry::stored("bad.soc", "invalid chunk file"))
        .entry(TestEntry::stored(
            "valid.soc",
            TestChunkFile::crch().bytes(),
        ));
    ArchiveBuilder::new()
        .entry(TestEntry::stored("loot.socpak", container.build()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;

    let (counts, ledger) =
        crate::catalog_read::original::with_entry_capture(&archive, |capture| {
            super::read(&archive, 0, None, capture)
        })?;

    assert!(
        counts.is_err(),
        "a later success cannot erase an earlier failure"
    );
    let [outer, failed, valid] = ledger.entries.as_slice() else {
        return Err("every selected source must be observed".into());
    };
    assert_eq!(
        outer.operation,
        EntryOperation::ContainerDirectory,
        "actual outer operation"
    );
    assert_eq!(
        failed.disposition,
        SourceDisposition::ProjectionFailed,
        "decoded bytes are not a SOC"
    );
    assert_eq!(
        valid.disposition,
        SourceDisposition::Projected,
        "valid zero-entity SOC succeeds"
    );
    assert_eq!(
        valid.operation,
        EntryOperation::SocTags,
        "actual SOC count operation"
    );
    Ok(())
}
