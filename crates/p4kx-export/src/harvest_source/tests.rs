// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};

use crate::export_ledger::SourceDisposition;
use crate::harvest_locations::Soc;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn opened(payload: Vec<u8>) -> Result<(tempfile::TempDir, Archive), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("unrelated.txt", "x"))
        .entry(TestEntry::stored("harvest.socpak", payload))
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}

#[test]
fn an_empty_container_is_a_successful_source_without_an_invented_soc() -> TestResult {
    let (_folder, archive) = opened(TestContainer::new().build())?;
    let (soc, ledger) = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        super::read(&archive, 1, capture)
    })?;
    assert_eq!(soc, Soc::Plain, "empty is a valid source result");
    let [outer] = ledger.entries.as_slice() else {
        return Err("one actual source Entry expected".into());
    };
    assert_eq!(outer.entry_index, Some(1), "actual outer index");
    assert_eq!(
        outer.container_entry_index, None,
        "no invented nested Entry"
    );
    assert_eq!(
        outer.disposition,
        SourceDisposition::Projected,
        "successful zero-row projection"
    );
    Ok(())
}

#[test]
fn a_failed_outer_open_does_not_claim_an_inner_read() -> TestResult {
    let (_folder, archive) = opened(b"invalid container".to_vec())?;
    let (soc, ledger) = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        super::read(&archive, 1, capture)
    })?;
    assert!(matches!(soc, Soc::Unreadable(_)), "the container failed");
    let [outer] = ledger.entries.as_slice() else {
        return Err("one failed source Entry expected".into());
    };
    assert_eq!(
        outer.path.as_str(),
        "harvest.socpak",
        "exact failed identity"
    );
    assert_eq!(
        outer.disposition,
        SourceDisposition::ReadFailed,
        "actual open failure"
    );
    Ok(())
}

#[test]
fn only_the_selected_first_soc_is_observed() -> TestResult {
    let container = TestContainer::new()
        .entry(TestEntry::stored("note.txt", "ignored"))
        .entry(TestEntry::stored("first.soc", "plain source"))
        .entry(TestEntry::zstd("second.soc", "unread source").with_payload(b"bad".to_vec()));
    let (_folder, archive) = opened(container.build())?;
    let (soc, ledger) = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        super::read(&archive, 1, capture)
    })?;
    assert_eq!(soc, Soc::Plain, "a successful scan can find no preset");
    let [outer, inner] = ledger.entries.as_slice() else {
        return Err("outer plus selected SOC expected".into());
    };
    assert_eq!(
        outer.disposition,
        SourceDisposition::Projected,
        "container opened"
    );
    assert_eq!(
        inner.container_entry_index,
        Some(1),
        "actual inner index, not filtered position"
    );
    assert_eq!(
        inner.path.as_str(),
        "harvest.socpak!first.soc",
        "nested identity survives"
    );
    assert_eq!(
        inner.disposition,
        SourceDisposition::Projected,
        "unused corrupt SOC is not read"
    );
    Ok(())
}

#[test]
fn a_corrupt_selected_soc_retains_its_nested_identity() -> TestResult {
    let container = TestContainer::new()
        .entry(TestEntry::zstd("bad.soc", "expected source").with_payload(b"bad".to_vec()));
    let (_folder, archive) = opened(container.build())?;
    let (soc, ledger) = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        super::read(&archive, 1, capture)
    })?;
    assert!(matches!(soc, Soc::Unreadable(_)), "selected source failed");
    let [outer, inner] = ledger.entries.as_slice() else {
        return Err("actual outer and inner operations expected".into());
    };
    assert_eq!(
        outer.disposition,
        SourceDisposition::Projected,
        "directory read succeeded"
    );
    assert_eq!(
        inner.path.as_str(),
        "harvest.socpak!bad.soc",
        "failed inner identity"
    );
    assert_eq!(
        inner.disposition,
        SourceDisposition::ReadFailed,
        "source decoder failed"
    );
    Ok(())
}
