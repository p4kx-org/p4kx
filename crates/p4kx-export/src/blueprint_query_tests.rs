// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_extraction::{
    Archive, CanonicalReadObserver, DataForgeEntry, DataForgeError, ProvenanceMode, Record,
    RecordTree,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::read::Query;
use crate::catalog_queries::CatalogRecordSelection;
use crate::catalog_query_session::CatalogQuerySource;

#[path = "blueprint_query_trace_fixture.rs"]
mod fixture;
use fixture::Product;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Default)]
struct Capture(Mutex<Vec<(u32, bool)>>);

impl CanonicalReadObserver for Capture {
    fn observe(
        &self,
        record: &Record<'_>,
        outcome: Result<&RecordTree<'_>, &DataForgeError>,
    ) -> ControlFlow<()> {
        let Ok(mut reads) = self.0.lock() else {
            return ControlFlow::Break(());
        };
        reads.push((record.index(), outcome.is_ok()));
        ControlFlow::Continue(())
    }
}

struct Summary {
    definitions: usize,
    failures: Vec<u32>,
    dismantle_present: Vec<bool>,
    reads: Vec<(u32, bool)>,
}

fn observe(
    product: Product,
    broken: bool,
    alias: bool,
    selection: CatalogRecordSelection,
) -> TestResult<Summary> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("source.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            fixture::database(product, broken, alias).build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let capture = Arc::new(Capture::default());
    let query = Query {
        archive: &archive,
        entry: CatalogQuerySource::new(&entry, ProvenanceMode::Full),
        forge: &forge,
        selection,
        languages: &[],
        cancel: None,
        observer: Some(capture.clone()),
    };
    let mut summary = query.run(|snapshot| Summary {
        definitions: snapshot.definitions.len(),
        failures: snapshot.coverage.unreadable_bodies,
        dismantle_present: snapshot
            .presentations
            .iter()
            .map(|row| row.dismantle().value().is_some())
            .collect(),
        reads: Vec::new(),
    })?;
    summary.reads = std::mem::take(
        &mut *capture
            .0
            .lock()
            .map_err(|_| "source read observer poisoned")?,
    );
    Ok(summary)
}

#[test]
fn missing_or_blacklisted_products_do_not_read_any_dismantle_body() -> TestResult {
    for product in [Product::Missing, Product::Blacklisted] {
        let result = observe(product, false, false, CatalogRecordSelection::All)?;
        assert_eq!(
            result.definitions, 1,
            "The selected complete Blueprint remains available"
        );
        assert_eq!(
            result.dismantle_present,
            [false],
            "The canonical early return excludes dismantling"
        );
        assert_eq!(
            result.reads,
            [(0, true), (1, true)],
            "Only the Blueprint and required globals were read"
        );
    }
    Ok(())
}

#[test]
fn selected_blueprints_read_needed_process_once_and_no_foreign_bodies() -> TestResult {
    let result = observe(Product::Present, false, true, CatalogRecordSelection::All)?;
    assert_eq!(
        result.definitions, 2,
        "Both physical GUID aliases are preserved"
    );
    assert_eq!(
        result.dismantle_present,
        [true, true],
        "Both aliases use the canonical generic process"
    );
    assert_eq!(
        result.reads,
        [(0, true), (5, true), (1, true), (2, true)],
        "Required dependencies are cached and Entity/foreign bodies remain unread"
    );
    Ok(())
}

#[test]
fn absent_or_failed_blueprint_owners_do_not_read_any_rule_bodies() -> TestResult {
    let absent = observe(
        Product::Present,
        false,
        false,
        CatalogRecordSelection::RecordIndex(3),
    )?;
    assert_eq!(
        absent.definitions, 0,
        "A foreign physical index has no Blueprint definition"
    );
    assert_eq!(
        absent.reads.len(),
        0,
        "Foreign selection reads no source bodies"
    );
    let failed = observe(Product::Present, true, false, CatalogRecordSelection::All)?;
    assert_eq!(
        failed.definitions, 0,
        "An unreadable selected owner is not fabricated"
    );
    assert_eq!(
        failed.failures,
        [0],
        "Owner body failure retains its physical index"
    );
    assert_eq!(
        failed.reads,
        [(0, false)],
        "No usable owner means no global or dismantle reads"
    );
    Ok(())
}

#[path = "blueprint_query_header_tests.rs"]
mod header_tests;
