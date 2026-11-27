// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_extraction::{
    Archive, CanonicalReadObserver, DataForgeEntry, DataForgeError, ProvenanceMode, Record,
    RecordTree,
};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType};

use super::read::Query;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::CatalogQuerySource;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Default)]
struct Capture(Mutex<Vec<(u32, bool)>>);

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
fn original_crafting_global_selection_never_reads_blueprint_or_resource_bodies() -> TestResult {
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
            "only original selected physical globals are read"
        );
        assert_eq!(rows, expected.len(), "same-GUID aliases remain separate");
        assert!(
            coverage.unreadable_bodies.is_empty(),
            "no broken dependency bodies opened"
        );
        assert_eq!(
            coverage.skipped_leftovers, 0,
            "globals have no invented leftover policy"
        );
    }
    Ok(())
}

#[test]
fn failed_crafting_global_owners_never_trigger_dependency_fallbacks() -> TestResult {
    let Observation {
        rows,
        coverage,
        reads,
    } = observe(true, CatalogRecordSelection::All)?;
    assert_eq!(
        rows, 0,
        "failed original owners cannot fabricate a configuration"
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
    let source = TestStruct::new("CraftingGlobalParams").field("number", TestType::UInt64);
    DataForgeBuilder::new()
        .structure(if broken {
            source.declared_size(1)
        } else {
            source
        })
        .structure(
            TestStruct::new("CraftingBlueprintRecord")
                .field("bad", TestType::UInt64)
                .declared_size(1),
        )
        .structure(
            TestStruct::new("ResourceType")
                .field("bad", TestType::UInt64)
                .declared_size(1),
        )
        .record(TestRecord::new(
            1,
            "CraftingGlobalParams",
            "CraftingGlobalParams.Primary",
            "libs/foundry/records/crafting/globalparams/primary.xml",
        ))
        .record(TestRecord::new(
            1,
            "CraftingGlobalParams",
            "CraftingGlobalParams.test_alias",
            "libs/foundry/records/crafting/globalparams/test_alias.xml",
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
        .record(TestRecord::new(
            4,
            "CraftingGlobalParams",
            "CraftingGlobalParams.ForeignPath",
            "libs/foundry/records/unrelated/foreign.xml",
        ))
}

fn observe(broken: bool, selection: CatalogRecordSelection) -> TestResult<Observation> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("source.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            database(broken).build(),
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
        cancel: None,
    };
    let (rows, coverage) = query.run(Some(capture.clone()), |snapshot| {
        (snapshot.rows.len(), snapshot.coverage)
    })?;
    let reads = std::mem::take(&mut *capture.0.lock().map_err(|_| "read observer poisoned")?);
    Ok(Observation {
        rows,
        coverage,
        reads,
    })
}
