// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_extraction::{
    Archive, CanonicalReadObserver, DataForgeEntry, DataForgeError, Record, RecordTree,
};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};

use super::BlueprintPoolSnapshot;
use super::pool_read::{self, Query};
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection, ParsedCatalogQueries};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Default)]
struct Capture(Mutex<Vec<(u32, bool)>>);

impl Capture {
    fn take(&self) -> TestResult<Vec<(u32, bool)>> {
        Ok(std::mem::take(
            &mut *self.0.lock().map_err(|_| "read observer poisoned")?,
        ))
    }
}

pub(crate) fn observe_pool_query<T>(
    parsed: &ParsedCatalogQueries<'_>,
    selection: CatalogRecordSelection,
    inspect: impl FnOnce(BlueprintPoolSnapshot<'_>) -> T,
) -> TestResult<(T, Vec<(u32, bool)>)> {
    let capture = Arc::new(Capture::default());
    let result = parsed
        .project(None, |archive, entry, forge| {
            pool_read::with_parsed(
                archive,
                entry,
                forge,
                selection,
                None,
                Some(capture.clone()),
                inspect,
            )
        })
        .map_err(|_| "observed Blueprint-pool SDK query failed")?;
    Ok((result, capture.take()?))
}

struct Observation {
    rows: usize,
    coverage: CatalogRecordCoverage,
    reads: Vec<(u32, bool)>,
}

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

#[test]
fn original_pool_selection_never_reads_reward_blueprint_global_or_resource_bodies() -> TestResult {
    for (selection, expected) in [
        (CatalogRecordSelection::All, vec![(0, true), (1, true)]),
        (CatalogRecordSelection::RecordIndex(1), vec![(1, true)]),
        (
            CatalogRecordSelection::Guid("00000000-0000-0000-0000-000000000001".parse()?),
            vec![(0, true), (1, true)],
        ),
        (CatalogRecordSelection::RecordIndex(2), vec![]),
    ] {
        let Observation {
            rows,
            coverage,
            reads,
        } = observe(false, selection)?;
        assert_eq!(
            reads, expected,
            "only original selected physical pools are read"
        );
        assert_eq!(rows, expected.len(), "same-GUID aliases remain separate");
        assert!(
            coverage.unreadable_bodies.is_empty(),
            "no broken dependency bodies opened"
        );
        assert_eq!(
            coverage.skipped_leftovers, 0,
            "canonical aliases remain separate from development leftovers"
        );
    }
    Ok(())
}

#[test]
fn failed_pool_owners_never_trigger_dependency_fallbacks() -> TestResult {
    let Observation {
        rows,
        coverage,
        reads,
    } = observe(true, CatalogRecordSelection::All)?;
    assert_eq!(
        rows, 0,
        "failed original owners cannot fabricate a definition"
    );
    assert_eq!(
        reads,
        [(0, false), (1, false)],
        "only original owners are attempted"
    );
    assert_eq!(
        coverage.unreadable_bodies,
        [0, 1],
        "physical failure positions survive"
    );
    assert!(
        coverage.incomplete_fields.is_empty(),
        "body and field failure remain distinct"
    );
    Ok(())
}

fn database(broken: bool) -> DataForgeBuilder {
    let source = TestStruct::new("BlueprintPoolRecord").field("number", TestType::UInt64);
    DataForgeBuilder::new()
        .structure(if broken {
            source.declared_size(1)
        } else {
            source
        })
        .structure(unreadable_record_type("CraftingBlueprintRecord"))
        .structure(unreadable_record_type("ResourceType"))
        .record(TestRecord::new(
            1,
            "BlueprintPoolRecord",
            "BlueprintPoolRecord.Primary",
            "libs/foundry/records/crafting/blueprintrewards/primary.xml",
        ))
        .record(TestRecord::new(
            1,
            "BlueprintPoolRecord",
            "BlueprintPoolRecord.Alias",
            "libs/foundry/records/crafting/blueprintrewards/alias.xml",
        ))
        .record(TestRecord::new(
            2,
            "CraftingBlueprintRecord",
            "Blueprint.Broken",
            "libs/foundry/records/crafting/blueprints/crafting/broken.xml",
        ))
        .record(TestRecord::new(
            3,
            "ResourceType",
            "Resource.Broken",
            "libs/foundry/records/resourcetypes/broken.xml",
        ))
        .structure(unreadable_record_type("ContractGenerator"))
        .structure(unreadable_record_type("CraftingGlobalParams"))
        .record(TestRecord::new(
            5,
            "ContractGenerator",
            "Contract.Broken",
            "libs/foundry/records/contracts/broken.xml",
        ))
        .record(TestRecord::new(
            6,
            "CraftingGlobalParams",
            "Globals.Broken",
            "libs/foundry/records/crafting/globalparams/broken.xml",
        ))
        .record(TestRecord::new(
            4,
            "BlueprintPoolRecord",
            "BlueprintPoolRecord.ForeignPath",
            "libs/foundry/records/unrelated/foreign.xml",
        ))
}

fn unreadable_record_type(name: &str) -> TestStruct {
    TestStruct::new(name)
        .field("bad", TestType::UInt64)
        .declared_size(1)
}

fn observe(broken: bool, selection: CatalogRecordSelection) -> TestResult<Observation> {
    observe_database(&database(broken), selection, |_| {})
}

fn observe_database(
    database: &DataForgeBuilder,
    selection: CatalogRecordSelection,
    inspect: impl FnOnce(&BlueprintPoolSnapshot<'_>),
) -> TestResult<Observation> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("source.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let capture = Arc::new(Capture::default());
    let query = Query {
        archive: &archive,
        entry: &entry,
        forge: &forge,
        selection,
        cancel: None,
    };
    let (rows, coverage) = query.run(Some(capture.clone()), |snapshot| {
        inspect(&snapshot);
        (snapshot.rows.len(), snapshot.coverage)
    })?;
    let reads = capture.take()?;
    Ok(Observation {
        rows,
        coverage,
        reads,
    })
}

#[test]
fn pool_leftovers_keep_the_catalog_policy_without_opening_their_bodies() -> TestResult {
    let database = database(false).record(TestRecord::new(
        7,
        "BlueprintPoolRecord",
        "BlueprintPoolRecord.test_leftover",
        "libs/foundry/records/crafting/blueprintrewards/test_leftover.xml",
    ));
    let found = observe_database(&database, CatalogRecordSelection::All, |_| {})?;
    assert_eq!(
        found.reads,
        [(0, true), (1, true)],
        "only ordinary physical pool bodies"
    );
    assert_eq!(found.rows, 2, "physical aliases remain rows");
    assert_eq!(
        found.coverage.skipped_leftovers, 1,
        "actual catalog classifier counts the leftover"
    );
    Ok(())
}

#[test]
fn pool_wrong_storage_and_unknown_unreadable_values_never_claim_complete_coverage() -> TestResult {
    let database = DataForgeBuilder::new()
        .structure(
            TestStruct::new("BlueprintPoolRecord")
                .array("blueprintRewards", TestType::Bool)
                .field("futureFlag", TestType::Bool),
        )
        .record(
            TestRecord::new(
                1,
                "BlueprintPoolRecord",
                "BlueprintPoolRecord.Primary",
                "libs/foundry/records/crafting/blueprintrewards/primary.xml",
            )
            .value("futureFlag", TestValue::Raw(vec![2])),
        );
    let found = observe_database(&database, CatalogRecordSelection::All, |snapshot| {
        let [row] = snapshot.rows() else {
            panic!("one selected original pool");
        };
        assert_eq!(
            row.blueprint_rewards().state(),
            p4kx_extraction::SourceValueState::Invalid,
            "a known Bool array cannot supply reward-entry slots"
        );
        let roots: Vec<_> = row
            .additional_fields()
            .roots()
            .iter()
            .filter_map(|field| field.name().value().copied())
            .collect();
        assert_eq!(
            roots,
            ["blueprintRewards", "futureFlag"],
            "rejected and additional original fields remain available"
        );
    })?;
    assert_eq!(
        found.coverage.incomplete_fields,
        [0],
        "recoverable original field failure is retained"
    );
    assert_eq!(
        found.coverage.unreadable_bodies.len(),
        0,
        "field errors are distinct from body failures"
    );
    Ok(())
}
