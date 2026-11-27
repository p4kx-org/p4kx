// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::atomic::Ordering;

use p4kx_archive::{Archive, SourceEncryption};
use p4kx_assembly::{ArchiveFiles, Entities, LoadoutFileError, LoadoutFiles, LoadoutResolver};
use p4kx_dataforge::DataForge;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::loadout_original_controls_support::{self as support, GenericFiles, Probe, TestResult};

#[test]
fn original_loadout_archive_guards_precede_payload_opening() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let size = p4kx_assembly::MAX_XML_LOADOUT_LEN + 1;
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/synthetic/plain.xml",
            b"<Loadout/>".to_vec(),
        ))
        .entry(TestEntry::stored("Data/synthetic/locked.xml", b"<Loadout/>".to_vec()).encrypted())
        .entry(
            TestEntry::stored("Data/synthetic/huge.xml", Vec::new())
                .with_uncompressed_size(size)
                .flagged_encrypted(),
        )
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let entries = archive.entries();
    assert_eq!(
        entries.len(),
        3,
        "all three original directory entries are admitted"
    );
    let files = ArchiveFiles::new(&archive);
    assert_eq!(
        files.read("SYNTHETIC/plain.xml")?,
        b"<Loadout/>",
        "existing path admission is unchanged"
    );
    assert!(
        matches!(
            files.read("synthetic/locked.xml"),
            Err(LoadoutFileError::Unreadable { .. })
        ),
        "the original encrypted payload cannot be opened without its key"
    );
    assert!(
        matches!(files.read("synthetic/huge.xml"), Err(LoadoutFileError::TooLarge { size: found }) if found == size),
        "the declared size guard wins before the missing encryption key is consulted"
    );
    assert!(
        matches!(
            files.read("synthetic/missing.xml"),
            Err(LoadoutFileError::NotFound)
        ),
        "a missing configured target remains distinct from an unreadable admitted Entry"
    );
    Ok(())
}

#[test]
fn original_loadout_container_read_keeps_plain_leaf_and_encrypted_ancestor_distinct() -> TestResult
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let container = ArchiveBuilder::new().entry(TestEntry::stored(
        "Data/synthetic/inside.xml",
        b"<Loadout/>".to_vec(),
    ));
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/ship.socpak", container.build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    let parent = archive
        .entries()
        .first()
        .ok_or("the original encrypted container is absent")?;
    let child = archive.open_container(parent)?;
    let leaf = child
        .entries()
        .first()
        .ok_or("the original plain XML leaf is absent")?;
    let origin = child
        .entry_origin(leaf)
        .ok_or("the actual child Entry has no original proof")?;
    assert_eq!(
        origin.entry().encryption(),
        SourceEncryption::Unencrypted,
        "the actual leaf is plain"
    );
    assert_eq!(
        origin.encryption(),
        SourceEncryption::Encrypted,
        "the actual containing source is encrypted"
    );
    assert_eq!(
        origin.ancestors().len(),
        1,
        "exactly the original containing step is retained"
    );
    assert_eq!(
        ArchiveFiles::new(&child).read("synthetic/inside.xml")?,
        b"<Loadout/>",
        "the actual child reader still returns the same plain leaf bytes"
    );
    Ok(())
}

#[test]
fn original_loadout_shared_failed_document_has_one_read_and_projection() -> TestResult {
    support::resolve_shared(true)
}

#[test]
fn original_loadout_unobserved_cache_keeps_the_same_results_without_callbacks() -> TestResult {
    support::resolve_shared(false)
}

#[test]
fn original_loadout_callback_stop_prevents_parse_and_future_reads() -> TestResult {
    let bytes = support::database();
    let forge = DataForge::open(&bytes)?;
    let originals = support::callers(&forge)?;
    let files = GenericFiles::default();
    let probe = Arc::new(Probe {
        stop_on_read: true,
        ..Probe::default()
    });
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), probe.clone())
            .with_files(&files);
    for record in originals {
        assert!(
            matches!(
                resolver.resolve(&record),
                Err(p4kx_assembly::LoadoutError::ObservationStopped)
            ),
            "the original callback stop remains fatal for every subsequent caller"
        );
    }
    assert_eq!(
        files.reads.load(Ordering::Relaxed),
        1,
        "a stopped reader performs no second physical read"
    );
    assert_eq!(
        probe.reads.load(Ordering::Relaxed),
        1,
        "the original completion is observed once"
    );
    assert_eq!(
        probe.projections.load(Ordering::Relaxed),
        0,
        "callback cancellation occurs before parsing"
    );
    Ok(())
}
