// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_extraction::{
    Archive, CanonicalReadObserver, DataForgeEntry, DataForgeError, Language, Record, RecordTree,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::parents::Observers;
use super::request::Query;
use super::snapshot::{ResourceGroupParentSnapshot, ResourceGroupSnapshot};
use crate::catalog_queries::CatalogRecordSelection;

#[path = "resource_group_query_trace_fixture.rs"]
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
        let Ok(mut reads) = self.0.lock() else {
            return ControlFlow::Break(());
        };
        reads.push((record.index(), outcome.is_ok()));
        ControlFlow::Continue(())
    }
}

#[derive(Clone, Copy)]
enum Projection {
    Direct,
    Parents,
}

struct Summary {
    owners: Vec<u32>,
    parent_targets: Vec<Option<u32>>,
    dependency_failures: Vec<u32>,
    reads: Vec<(u32, bool)>,
    key_before: usize,
    key_after: usize,
}

impl Summary {
    fn new(owners: Vec<u32>, key_before: usize) -> Self {
        Self {
            owners,
            parent_targets: Vec::new(),
            dependency_failures: Vec::new(),
            reads: Vec::new(),
            key_before,
            key_after: 0,
        }
    }
}

fn open(broken: bool, alias: bool) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("source.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            fixture::database(broken, alias).build(),
        ))
        .entry(TestEntry::stored(Language::English.global_ini_path(), b"shared=secret").encrypted())
        .entry(
            TestEntry::stored(
                "Data/ObjectContainers/PU/system/synthetic/syntheticsystem.socpak",
                b"locked-world",
            )
            .encrypted(),
        )
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}

fn observe(
    projection: Projection,
    selection: CatalogRecordSelection,
    broken: bool,
    alias: bool,
) -> TestResult<Summary> {
    let (_folder, archive) = open(broken, alias)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let key_before = archive.key_demand().snapshot().unread();
    let capture = Arc::new(Capture::default());
    let query = Query {
        archive: &archive,
        entry: &entry,
        forge: &forge,
        selection,
        languages: if matches!(projection, Projection::Parents) {
            &[Language::English]
        } else {
            &[]
        },
        cancel: None,
    };
    let mut result = match projection {
        Projection::Direct => query.run_direct_observed(Some(capture.clone()), |snapshot| {
            direct_summary(&snapshot, key_before)
        })??,
        Projection::Parents => query.run_parents_observed(
            Observers {
                owners: Some(capture.clone()),
                groups: Some(capture.clone()),
            },
            |snapshot| parent_summary(&snapshot, key_before),
        )??,
    };
    result.reads = std::mem::take(&mut *capture.0.lock().map_err(|_| "source observer poisoned")?);
    result.key_after = archive.key_demand().snapshot().unread();
    Ok(result)
}

fn direct_summary(snapshot: &ResourceGroupSnapshot<'_>, key_before: usize) -> TestResult<Summary> {
    let owners = snapshot
        .definitions()
        .iter()
        .map(|row| {
            row.header()
                .record_index()
                .value()
                .copied()
                .ok_or("physical group owner")
        })
        .collect::<Result<_, _>>()?;
    Ok(Summary::new(owners, key_before))
}

fn parent_summary(
    snapshot: &ResourceGroupParentSnapshot,
    key_before: usize,
) -> TestResult<Summary> {
    let owners = snapshot
        .rows()
        .iter()
        .map(|row| {
            row.record_index()
                .value()
                .copied()
                .ok_or("physical child owner")
        })
        .collect::<Result<_, _>>()?;
    let mut result = Summary::new(owners, key_before);
    result.parent_targets = snapshot
        .rows()
        .iter()
        .map(|row| {
            row.parent()
                .value()
                .and_then(Option::as_ref)
                .and_then(|reference| reference.target_record_index().value().copied())
        })
        .collect();
    result.dependency_failures = snapshot.dependency_coverage().unreadable_bodies.clone();
    Ok(result)
}

#[test]
fn direct_group_query_reads_only_selected_owner_and_no_resource_or_world_payload() -> TestResult {
    let result = observe(
        Projection::Direct,
        CatalogRecordSelection::RecordIndex(0),
        false,
        false,
    )?;
    assert_eq!(result.owners, [0], "One selected physical group is exposed");
    assert_eq!(
        result.reads,
        [(0, true)],
        "The referenced resource and unrelated group bodies stay unread"
    );
    assert_eq!(
        result.key_after, result.key_before,
        "No encrypted locale or world payload was requested"
    );
    Ok(())
}

#[test]
fn parent_query_reuses_owner_body_and_reads_only_canonical_group_dependencies() -> TestResult {
    let result = observe(
        Projection::Parents,
        CatalogRecordSelection::RecordIndex(0),
        false,
        false,
    )?;
    assert_eq!(
        result.owners,
        [0],
        "Parent output is scoped to the selected child"
    );
    assert_eq!(
        result.parent_targets,
        [Some(1)],
        "The physical parent outside owner selection is retained"
    );
    assert_eq!(
        result.reads,
        [(0, true), (1, true)],
        "The selected tree is reused and no resource or foreign body is read"
    );
    assert_eq!(
        result.key_after, result.key_before,
        "Even an attached requested locale is not read by parent projection"
    );
    Ok(())
}

#[test]
fn equal_guid_group_owners_remain_distinct_and_share_one_physical_parent_survey() -> TestResult {
    let selection = CatalogRecordSelection::Guid("00000000-0000-0000-0000-000000000001".parse()?);
    let result = observe(Projection::Parents, selection, false, true)?;
    assert_eq!(
        result.owners,
        [0, 4],
        "GUID selection retains both physical child aliases"
    );
    assert_eq!(
        result.parent_targets,
        [Some(1), Some(1)],
        "Both aliases use canonical external-GUID parent policy"
    );
    assert_eq!(
        result.reads,
        [(0, true), (4, true), (1, true)],
        "Each needed group body is built once"
    );
    Ok(())
}

#[test]
fn missing_or_failed_group_owners_do_not_trigger_parent_family_or_locale_reads() -> TestResult {
    let foreign = observe(
        Projection::Parents,
        CatalogRecordSelection::RecordIndex(2),
        false,
        false,
    )?;
    assert_eq!(foreign.owners.len(), 0, "A resource is not a group owner");
    assert_eq!(
        foreign.reads.len(),
        0,
        "A foreign selection does not trigger any body read"
    );
    let failed = observe(
        Projection::Parents,
        CatalogRecordSelection::RecordIndex(0),
        true,
        false,
    )?;
    assert_eq!(
        failed.owners.len(),
        0,
        "An unreadable selected owner is not fabricated"
    );
    assert_eq!(
        failed.reads,
        [(0, false)],
        "No usable owner means no dependency body reads"
    );
    assert_eq!(
        failed.dependency_failures.len(),
        0,
        "The unrequested dependency survey is not misreported as attempted"
    );
    assert_eq!(
        failed.key_after, failed.key_before,
        "Failed owners do not demand encrypted locale or world entries"
    );
    Ok(())
}
