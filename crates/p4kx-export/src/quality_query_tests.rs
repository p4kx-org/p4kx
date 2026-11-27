// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_extraction::{
    Archive, CanonicalReadObserver, DataForgeEntry, DataForgeError, Guid, ProvenanceMode, Record,
    RecordTree,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::read::Query;
use super::rows::{Distributions, Projector, Quantizations};
use super::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::CatalogQuerySource;

#[path = "quality_query_trace_fixture.rs"]
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
        let Ok(mut entries) = self.0.lock() else {
            return ControlFlow::Break(());
        };
        entries.push((record.index(), outcome.is_ok()));
        ControlFlow::Continue(())
    }
}

struct Summary {
    rows: usize,
    coverage: CatalogRecordCoverage,
    reads: Vec<(u32, bool)>,
}

fn observe<P: Projector>(
    selection: CatalogRecordSelection,
    broken_distribution: bool,
) -> TestResult<Summary> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("source.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            fixture::database(broken_distribution).build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let capture = Arc::new(Capture::default());
    let mut summary = Query {
        archive: &archive,
        entry: CatalogQuerySource::new(&entry, ProvenanceMode::Full),
        forge: &forge,
        selection,
        cancel: None,
        observer: Some(capture.clone()),
    }
    .run::<P, _>(|snapshot| Summary {
        rows: snapshot.rows().len(),
        coverage: snapshot.coverage,
        reads: Vec::new(),
    })?;
    summary.reads = std::mem::take(
        &mut *capture
            .0
            .lock()
            .map_err(|_| "canonical read observer poisoned")?,
    );
    Ok(summary)
}

#[test]
fn selected_quality_queries_read_only_the_requested_physical_owner_bodies() -> TestResult {
    let distributions = observe::<Distributions>(CatalogRecordSelection::Guid(guid(1)?), false)?;
    assert_eq!(distributions.rows, 2, "both physical GUID aliases survive");
    assert_eq!(
        distributions.reads,
        [(0, true), (7, true)],
        "no sibling distribution, quantizer, Resource or foreign body is read"
    );
    let quantization = observe::<Quantizations>(CatalogRecordSelection::Guid(guid(3)?), false)?;
    assert_eq!(
        quantization.rows, 1,
        "the selected raw quantization remains independent of inverse resource lookup"
    );
    assert_eq!(
        quantization.reads,
        [(2, true)],
        "quantization target references never request a Resource-body census"
    );
    Ok(())
}

#[test]
fn complete_quality_family_reads_include_both_distribution_owner_types_and_skip_leftovers()
-> TestResult {
    let distributions = observe::<Distributions>(CatalogRecordSelection::All, false)?;
    assert_eq!(
        distributions.rows, 4,
        "both distributions and location overrides belong to the canonical family"
    );
    assert_eq!(
        distributions.reads,
        [(0, true), (1, true), (6, true), (7, true)],
        "unrelated bodies and known leftovers are never materialized"
    );
    assert_eq!(
        distributions.coverage.skipped_leftovers, 1,
        "the source policy accounts for its actual skipped leftover"
    );
    let quantizations = observe::<Quantizations>(CatalogRecordSelection::All, false)?;
    assert_eq!(
        quantizations.rows, 2,
        "raw null quantization owners remain addressable"
    );
    assert_eq!(
        quantizations.reads,
        [(2, true), (3, true)],
        "the direct family query computes no inverse dependencies"
    );
    Ok(())
}

#[test]
fn foreign_and_failed_owner_selections_never_trigger_other_quality_or_resource_bodies() -> TestResult
{
    let foreign = observe::<Distributions>(CatalogRecordSelection::RecordIndex(4), false)?;
    assert_eq!(
        foreign.rows, 0,
        "a Resource physical index is outside the requested family"
    );
    assert!(
        foreign.reads.is_empty(),
        "foreign selection reads no canonical body"
    );
    let failed = observe::<Distributions>(CatalogRecordSelection::Guid(guid(1)?), true)?;
    assert_eq!(
        failed.rows, 0,
        "unreadable original owners are never fabricated"
    );
    assert_eq!(
        failed.reads,
        [(0, false), (7, false)],
        "failed selected owners do not trigger any fallback or dependency scan"
    );
    assert_eq!(
        failed.coverage.unreadable_bodies,
        [0, 7],
        "failure coverage retains both exact physical identities"
    );
    assert!(
        failed.coverage.incomplete_fields.is_empty(),
        "a failed body is distinct from a retained row's field issue"
    );
    Ok(())
}

fn guid(value: u128) -> TestResult<Guid> {
    Ok(format!("00000000-0000-0000-0000-{value:012x}").parse()?)
}
