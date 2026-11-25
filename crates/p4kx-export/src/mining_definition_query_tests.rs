// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_extraction::{
    Archive, CanonicalReadObserver, DataForgeEntry, DataForgeError, ProvenanceMode, Record,
    RecordTree,
};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry};

use super::read::{Observers, Query};
use super::rows::{Compositions, Elements, Projector, Resources};
use super::snapshot::MiningDefinitionSnapshot;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::CatalogQuerySource;

#[path = "mining_definition_query_trace_fixture.rs"]
mod fixture;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Default)]
struct Capture(Mutex<Vec<(u32, bool)>>);

impl CanonicalReadObserver for Capture {
    fn observe(
        &self,
        record: &Record<'_>,
        outcome: Result<&RecordTree<'_>, &DataForgeError>,
    ) -> ControlFlow<()> {
        let Ok(mut rows) = self.0.lock() else {
            return ControlFlow::Break(());
        };
        rows.push((record.index(), outcome.is_ok()));
        ControlFlow::Continue(())
    }
}

struct ResultSummary {
    rows: usize,
    coverage: CatalogRecordCoverage,
    dependencies: CatalogRecordCoverage,
    reads: Vec<(u32, bool)>,
}

fn observe<P: Projector>(
    database: &DataForgeBuilder,
    selection: CatalogRecordSelection,
    inspect: impl for<'source> FnOnce(&MiningDefinitionSnapshot<'source, P::Row<'source>>),
) -> TestResult<ResultSummary> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("source.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let capture = Arc::new(Capture::default());
    let observers = Observers {
        owners: Some(capture.clone()),
        groups: Some(capture.clone()),
    };
    let query = Query {
        archive: &archive,
        entry: CatalogQuerySource::new(&entry, ProvenanceMode::Full),
        forge: &forge,
        selection,
        requested_languages: &[],
        cancel: None,
    };
    let (rows, coverage, dependencies) = query.run::<P, _>(observers, |snapshot| {
        inspect(&snapshot);
        let MiningDefinitionSnapshot {
            rows,
            coverage,
            dependency_coverage,
            ..
        } = snapshot;
        (rows.len(), coverage, dependency_coverage)
    })?;
    let reads = std::mem::take(&mut *capture.0.lock().map_err(|_| "read observer poisoned")?);
    Ok(ResultSummary {
        rows,
        coverage,
        dependencies,
        reads,
    })
}

#[test]
fn resources_read_only_selected_owners_and_required_reverse_groups_once() -> TestResult {
    let database = fixture::database(false, false);
    let result = observe::<Resources>(&database, CatalogRecordSelection::All, |_| {})?;
    assert_eq!(result.rows, 2, "All selected resource owners are returned");
    assert_eq!(
        result.reads,
        [(0, true), (1, true), (4, true), (5, true)],
        "Only resource owners and required groups are read"
    );
    assert_eq!(
        result.coverage.unreadable_bodies.len(),
        0,
        "Readable owners have no body failures"
    );
    assert_eq!(
        result.dependencies.unreadable_bodies.len(),
        0,
        "Readable group dependencies have no body failures"
    );
    let result = observe::<Resources>(&database, CatalogRecordSelection::RecordIndex(1), |_| {})?;
    assert_eq!(
        result.rows, 1,
        "Physical selection returns one resource owner"
    );
    assert_eq!(
        result.reads,
        [(1, true), (4, true), (5, true)],
        "Physical selection reads no sibling resource body"
    );
    Ok(())
}

#[test]
fn elements_and_compositions_never_read_resource_group_or_foreign_bodies() -> TestResult {
    let database = fixture::database(false, true);
    let elements = observe::<Elements>(&database, CatalogRecordSelection::All, |_| {})?;
    assert_eq!(elements.rows, 1, "The selected element is returned");
    assert_eq!(
        elements.reads,
        [(2, true)],
        "Element queries read only element bodies"
    );
    assert_eq!(
        elements.dependencies.unreadable_bodies.len(),
        0,
        "Element queries demand no group bodies"
    );
    let compositions = observe::<Compositions>(&database, CatalogRecordSelection::All, |_| {})?;
    assert_eq!(compositions.rows, 1, "The selected composition is returned");
    assert_eq!(
        compositions.reads,
        [(3, true)],
        "Composition queries read only composition bodies"
    );
    assert_eq!(
        compositions.dependencies.unreadable_bodies.len(),
        0,
        "Composition queries demand no group bodies"
    );
    Ok(())
}

#[test]
fn absent_and_failed_selected_owners_do_not_trigger_group_bodies() -> TestResult {
    let database = fixture::database(true, false);
    let absent = observe::<Resources>(&database, CatalogRecordSelection::RecordIndex(2), |_| {})?;
    assert_eq!(absent.rows, 0, "Foreign family selection yields no owner");
    assert_eq!(
        absent.reads.len(),
        0,
        "Foreign family selection reads no body"
    );
    let failed = observe::<Resources>(&database, CatalogRecordSelection::RecordIndex(0), |_| {})?;
    assert_eq!(failed.rows, 0, "An unreadable selected owner yields no row");
    assert_eq!(
        failed.reads,
        [(0, false)],
        "Only the failed owner body was attempted"
    );
    assert_eq!(
        failed.coverage.unreadable_bodies,
        [0],
        "Owner failures retain exact physical identity"
    );
    assert_eq!(
        failed.dependencies.unreadable_bodies.len(),
        0,
        "Failed owners demand no groups"
    );
    let all_failed = observe::<Resources>(&database, CatalogRecordSelection::All, |_| {})?;
    assert_eq!(all_failed.rows, 0, "All failed owners yield no rows");
    assert_eq!(
        all_failed.reads,
        [(0, false), (1, false)],
        "Only failed owner bodies were attempted"
    );
    assert_eq!(
        all_failed.coverage.unreadable_bodies,
        [0, 1],
        "Each failed physical owner is reported"
    );
    assert_eq!(
        all_failed.dependencies.unreadable_bodies.len(),
        0,
        "No readable owners means no group demand"
    );
    Ok(())
}

#[test]
fn required_group_failures_are_reported_separately_from_owner_reads() -> TestResult {
    let result = observe::<Resources>(
        &fixture::database(false, true),
        CatalogRecordSelection::RecordIndex(0),
        |snapshot| {
            for row in snapshot.rows() {
                assert!(
                    !row.group_guids().origin().source_set_complete(),
                    "A failed survey cannot prove complete membership"
                );
                assert_eq!(
                    row.group_guids().origin().contains_encrypted_source(),
                    None,
                    "Incomplete cleartext evidence remains unknown"
                );
            }
        },
    )?;
    assert_eq!(
        result.rows, 1,
        "Recoverable group failure retains its owner row"
    );
    assert_eq!(
        result.reads,
        [(0, true), (4, false), (5, false)],
        "Only the owner and necessary groups are attempted"
    );
    assert_eq!(
        result.coverage.unreadable_bodies.len(),
        0,
        "Group failure is separate from owner-body coverage"
    );
    assert_eq!(
        result.dependencies.unreadable_bodies,
        [4, 5],
        "Both failed group bodies retain physical identity"
    );
    Ok(())
}

#[path = "mining_process_query_trace.rs"]
mod processes;
