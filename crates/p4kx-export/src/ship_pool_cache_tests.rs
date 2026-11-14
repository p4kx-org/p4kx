// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{ArchiveFiles, Entities};
use p4kx_extraction::{Archive, DataForge, Entry, Record, RecordTree};

use super::{CatalogRead, ObservedReader, ShipPool};
use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::ship_pool_test_input;
use crate::build::original_input::test_support::TestResult;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_entry_ledger::{ArchiveEntryObservation, EntryOperation};
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::loadout::ledger::Capture;
use crate::ship_base::ShipReader;
use crate::{CatalogError, Kind};

#[path = "ship_pool_cache_fixture.rs"]
mod fixture;
#[path = "ship_pool_cache_probe.rs"]
mod probe;

use probe::Probe;

#[test]
fn sequential_ship_pool_preserves_successful_file_evidence_across_finish() -> TestResult {
    run(fixture::Case::Valid)
}

#[test]
fn sequential_ship_pool_preserves_failed_projection_evidence_across_finish() -> TestResult {
    run(fixture::Case::Malformed)
}

#[test]
fn sequential_ship_pool_preserves_failed_read_evidence_across_finish() -> TestResult {
    run(fixture::Case::Unreadable)
}

#[test]
fn sequential_ship_pool_preserves_missing_file_evidence_across_finish() -> TestResult {
    run(fixture::Case::Missing)
}

fn run(case: fixture::Case) -> TestResult {
    ship_pool_test_input::with_input(
        &fixture::database(),
        case.entries(),
        |archive, files, forge, input| check(archive, files, forge, input, case),
    )
}

fn check<'input>(
    archive: &'input Archive,
    files: &'input ArchiveFiles<'input>,
    forge: &'input DataForge<'input>,
    input: &'input OriginalCatalogInput<'input, 'input>,
    case: fixture::Case,
) -> TestResult {
    assert_eq!(
        forge.records().count(),
        2,
        "two actual source ships, no synthetic catalog rows"
    );
    let first = forge.record(0).ok_or("actual first Record")?;
    let second = forge.record(1).ok_or("actual second Record")?;
    assert_ne!(
        first.guid(),
        second.guid(),
        "the real callers are physically distinct"
    );
    let first_tree = first.tree()?;
    let second_tree = second.tree()?;
    original(&first, &first_tree)?;
    original(&second, &second_tree)?;
    let first_source = source(&first, &first_tree)?;
    let observed = observed_pool(files, forge, input, &first_source);
    let first_read = observed
        .pool
        .read_catalog(&first, &first_tree, &first_source, input, None)?;
    first_result(&first_read, archive, case, &observed)?;
    let second_source = source(&second, &second_tree)?;
    let second_read =
        observed
            .pool
            .read_catalog(&second, &second_tree, &second_source, input, None);
    observed.probe.second(archive, case)?;
    let second_read = second_read?;
    assert!(
        second_read.read.is_ok(),
        "the second original row survives real cached outcomes"
    );
    assert!(
        second_read.ledger.entries.is_empty(),
        "a genuine cache reuse emits no new byte-read or projection operation"
    );
    entries(&first_read.ledger, archive, case)?;
    same_capture(&observed)?;
    Ok(())
}

struct Observed<'input> {
    pool: ShipPool<'input>,
    probe: Arc<Probe<'input>>,
    capture: Arc<Capture<'input>>,
}

fn observed_pool<'input>(
    files: &'input ArchiveFiles<'input>,
    forge: &'input DataForge<'input>,
    input: &'input OriginalCatalogInput<'input, 'input>,
    source: &ExportLedger,
) -> Observed<'input> {
    let entities = Arc::new(Entities::new(forge));
    let capture = Arc::new(Capture::for_input(input, source, None, Kind::Ships));
    let probe = Arc::new(Probe::new(Arc::clone(&capture)));
    let pool = ShipPool::new(Arc::clone(&entities), files);
    pool.observed
        .lock()
        .expect("the actual reader pool is not poisoned")
        .push(ObservedReader {
            reader: ShipReader::with_observed_entities(entities, files, Arc::clone(&probe)),
            capture: Arc::clone(&capture),
        });
    Observed {
        pool,
        probe,
        capture,
    }
}

fn first_result(
    read: &CatalogRead<'_>,
    archive: &Archive,
    case: fixture::Case,
    observed: &Observed<'_>,
) -> TestResult {
    assert!(
        read.read.is_ok(),
        "the first genuine ShipReader returns its partial or full row"
    );
    observed.probe.first(archive, case)?;
    entries(&read.ledger, archive, case)?;
    let empty = observed.capture.finish()?;
    assert!(
        empty.entries.is_empty(),
        "the first caller's ledger delta was already drained"
    );
    Ok(())
}

fn same_capture(observed: &Observed<'_>) -> TestResult {
    let pool = observed
        .pool
        .observed
        .lock()
        .map_err(|_| "actual reader pool is poisoned")?;
    let returned = pool.first().ok_or("returned actual reader")?;
    assert!(
        Arc::ptr_eq(&returned.capture, &observed.capture),
        "both calls use the same real cached reader's capture"
    );
    drop(pool);
    Ok(())
}
fn original(record: &Record<'_>, tree: &RecordTree<'_>) -> TestResult {
    assert!(
        tree.is_from(record),
        "the actual tree is paired with its original Record"
    );
    assert_eq!(
        tree.error_count(),
        0,
        "no canonical source error masks the file-cache boundary"
    );
    assert_eq!(
        record.type_name()?,
        "EntityClassDefinition",
        "actual ship source type"
    );
    Ok(())
}

fn source(record: &Record<'_>, tree: &RecordTree<'_>) -> Result<ExportLedger, CatalogError> {
    crate::export_field_walk::inspect(
        tree,
        record,
        Some(Kind::Ships),
        ExportSourceRole::Selected,
        None,
    )
}

fn entries(ledger: &ExportLedger, archive: &Archive, case: fixture::Case) -> TestResult {
    let expected = if case.projected().is_some() { 2 } else { 1 };
    assert_eq!(
        ledger.entries.len(),
        expected,
        "only the genuine intrinsic file operations"
    );
    let read = ledger
        .entries
        .iter()
        .find(|entry| entry.operation == EntryOperation::XmlLoadoutBytes)
        .ok_or("retained real byte-read operation")?;
    read_entry(read, archive, case);
    let projection = ledger
        .entries
        .iter()
        .find(|entry| entry.operation == EntryOperation::XmlLoadout);
    match (case.projected(), projection) {
        (Some(expected), Some(projected)) => assert_eq!(
            projected.disposition,
            if expected {
                SourceDisposition::Projected
            } else {
                SourceDisposition::ProjectionFailed
            },
            "the same original intrinsic projection"
        ),
        (None, None) => {}
        _ => return Err("the real intrinsic projection applicability changed".into()),
    }
    Ok(())
}

fn read_entry(read: &ArchiveEntryObservation, archive: &Archive, case: fixture::Case) {
    let actual = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == fixture::PATH);
    assert_eq!(
        read.entry_index,
        actual.map(Entry::index),
        "actual source position only"
    );
    assert_eq!(
        read.kind,
        Kind::Ships,
        "the current caller's declared source kind"
    );
    assert_eq!(
        read.role,
        ExportSourceRole::Dependency,
        "the genuine file dependency"
    );
    let expected = match case {
        fixture::Case::Valid | fixture::Case::Malformed => SourceDisposition::Projected,
        fixture::Case::Unreadable => SourceDisposition::ReadFailed,
        fixture::Case::Missing => SourceDisposition::SkippedBeforeRead,
    };
    assert_eq!(
        read.disposition, expected,
        "the original typed byte outcome is preserved"
    );
}
