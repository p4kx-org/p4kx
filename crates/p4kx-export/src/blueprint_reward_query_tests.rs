// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_extraction::{
    Archive, CancelToken, CanonicalReadObserver, DataForgeEntry, DataForgeError, ProvenanceMode,
    Record, RecordTree,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue};

use super::reward_read::Query;
use super::{BlueprintRewardSelection, BlueprintRewardSnapshot};
use crate::CatalogError;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::CatalogQuerySource;

#[path = "blueprint_reward_query_alias_tests.rs"]
mod aliases;
#[path = "blueprint_reward_query_fixture.rs"]
mod fixture;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Default)]
struct Capture {
    reads: Mutex<Vec<(u32, bool)>>,
    cancel: Option<Arc<CancelToken>>,
}

impl CanonicalReadObserver for Capture {
    fn observe(
        &self,
        record: &Record<'_>,
        outcome: Result<&RecordTree<'_>, &DataForgeError>,
    ) -> ControlFlow<()> {
        let Ok(mut reads) = self.reads.lock() else {
            return ControlFlow::Break(());
        };
        reads.push((record.index(), outcome.is_ok()));
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
        }
        ControlFlow::Continue(())
    }
}

struct Outcome {
    rows: Vec<(u32, u64, u32)>,
    source: CatalogRecordCoverage,
    pools: CatalogRecordCoverage,
    source_reads: usize,
    pool_reads: usize,
}

fn outcome(snapshot: BlueprintRewardSnapshot<'_>) -> TestResult<Outcome> {
    let mut rows = Vec::new();
    for row in snapshot.rows() {
        rows.push((
            *row.source()
                .record_index()
                .value()
                .ok_or("source physical index")?,
            row.source_ordinal().value().ok_or("source ordinal")?.get(),
            *row.pool()
                .value()
                .and_then(|pool| pool.target_record_index().value())
                .ok_or("actual physical pool")?,
        ));
    }
    Ok(Outcome {
        rows,
        source: snapshot.source_coverage,
        pools: snapshot.pool_coverage,
        source_reads: snapshot.source_records_read,
        pool_reads: snapshot.pool_records_read,
    })
}

fn query(
    bytes: Vec<u8>,
    selection: BlueprintRewardSelection,
) -> TestResult<(Outcome, Vec<(u32, bool)>)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("reward-sdk.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", bytes))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let capture = Arc::new(Capture::default());
    let result = Query {
        archive: &archive,
        entry: CatalogQuerySource::new(&entry, ProvenanceMode::Full),
        forge: &forge,
        selection,
        cancel: None,
    }
    .run(Some(capture.clone()), outcome)??;
    let reads = std::mem::take(&mut *capture.reads.lock().map_err(|_| "observer poisoned")?);
    Ok((result, reads))
}

#[test]
fn a_non_pool_target_opens_no_record_bodies() -> TestResult {
    let selection =
        BlueprintRewardSelection::TargetPools(CatalogRecordSelection::RecordIndex(fixture::SOURCE));
    let (found, reads) = query(fixture::database(true).build(), selection)?;
    assert_eq!(
        (found.source_reads, found.pool_reads),
        (0, 0),
        "empty survey scope"
    );
    assert!(
        found.rows.is_empty() && reads.is_empty(),
        "no target was admitted"
    );
    Ok(())
}

#[test]
fn selected_source_reads_only_demanded_physical_pool_dependencies() -> TestResult {
    let selection = BlueprintRewardSelection::SourceOwners(CatalogRecordSelection::RecordIndex(
        fixture::SOURCE,
    ));
    let (found, reads) = query(fixture::database(true).build(), selection)?;
    assert_eq!(
        reads,
        [(3, true), (0, true), (1, true)],
        "one source and its two actual pool aliases only"
    );
    assert_eq!(
        (found.source_reads, found.pool_reads),
        (1, 2),
        "actual body attempts"
    );
    assert_eq!(
        found.rows.len(),
        4,
        "direct, root weak and two owned occurrences; raw array excluded"
    );
    assert_eq!(
        found.rows.iter().map(|row| row.2).collect::<Vec<_>>(),
        [0, 1, 0, 0],
        "physical targets stay exact"
    );
    assert!(
        found.source.unreadable_bodies.is_empty() && found.pools.unreadable_bodies.is_empty(),
        "unrelated broken bodies stay unopened"
    );
    Ok(())
}

#[test]
fn inverse_physical_alias_selection_streams_all_sources_and_retains_real_gaps() -> TestResult {
    let selection =
        BlueprintRewardSelection::TargetPools(CatalogRecordSelection::RecordIndex(fixture::ALIAS));
    let (found, reads) = query(fixture::database(true).build(), selection)?;
    assert_eq!(
        (found.source_reads, found.pool_reads),
        (8, 2),
        "inverse cost is an explicit whole-source survey"
    );
    assert_eq!(
        found.rows.len(),
        1,
        "only the weak link reaches the selected actual alias"
    );
    let [row] = found.rows.as_slice() else {
        return Err("one physical target occurrence".into());
    };
    assert_eq!(
        (row.0, row.2),
        (fixture::SOURCE, fixture::ALIAS),
        "source and target physical identities"
    );
    assert_eq!(
        found.source.unreadable_bodies,
        [6, 7],
        "unseen failed types cannot disappear from inverse coverage"
    );
    assert_eq!(
        reads.iter().filter(|(_, ok)| !ok).count(),
        2,
        "actual failed body receipts"
    );
    Ok(())
}

#[test]
fn guid_source_selection_keeps_physical_aliases_and_original_ordinals() -> TestResult {
    let guid = "00000000-0000-0000-0000-000000000003".parse()?;
    let selection = BlueprintRewardSelection::SourceOwners(CatalogRecordSelection::Guid(guid));
    let (all, _) = query(fixture::database(false).build(), selection)?;
    let one = BlueprintRewardSelection::SourceOwners(CatalogRecordSelection::RecordIndex(
        fixture::SOURCE,
    ));
    let (one, _) = query(fixture::database(false).build(), one)?;
    assert_eq!(
        all.rows.len(),
        5,
        "same-GUID source owners remain independent"
    );
    assert_eq!(
        all.rows
            .iter()
            .filter(|row| row.0 == fixture::SOURCE)
            .copied()
            .collect::<Vec<_>>(),
        one.rows,
        "selection does not renumber original occurrences"
    );
    assert_eq!(
        all.rows.last().map(|row| row.0),
        Some(fixture::SOURCE_ALIAS),
        "physical source order"
    );
    Ok(())
}

#[test]
fn a_readable_zero_match_body_with_an_opaque_field_error_is_incomplete() -> TestResult {
    let database = fixture::database(false)
        .structure(TestStruct::new("NoHit").field("future", TestType::Bool))
        .record(
            TestRecord::new(40, "NoHit", "NoHit.Primary", "outside/nohit.xml")
                .value("future", TestValue::Raw(vec![2])),
        );
    let selection = BlueprintRewardSelection::SourceOwners(CatalogRecordSelection::RecordIndex(8));
    let (found, reads) = query(database.build(), selection)?;
    assert!(found.rows.is_empty(), "no qualifying observed Reference");
    assert_eq!(
        reads,
        [(8, true)],
        "the original body was readable and actually inspected"
    );
    assert_eq!(
        found.source.incomplete_fields,
        [8],
        "zero rows never erase the actual field-read gap"
    );
    assert_eq!(found.pool_reads, 0, "no fabricated target dependency");
    Ok(())
}

#[test]
fn cancellation_during_actual_source_read_prevents_consumer_invocation() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("cancel.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            fixture::database(false).build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let cancel = Arc::new(CancelToken::new());
    let capture = Arc::new(Capture {
        reads: Mutex::new(Vec::new()),
        cancel: Some(cancel.clone()),
    });
    let result = Query {
        archive: &archive,
        entry: CatalogQuerySource::new(&entry, ProvenanceMode::Full),
        forge: &forge,
        selection: BlueprintRewardSelection::SourceOwners(CatalogRecordSelection::RecordIndex(
            fixture::SOURCE,
        )),
        cancel: Some(&cancel),
    }
    .run(Some(capture), |_| {
        panic!("cancelled survey must not invoke a consumer")
    });
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "mid-read cancellation propagates"
    );
    Ok(())
}

#[path = "blueprint_reward_query_scope_tests.rs"]
mod scope;
