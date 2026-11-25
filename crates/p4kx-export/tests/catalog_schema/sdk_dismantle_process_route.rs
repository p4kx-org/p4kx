// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::crafting::{
    DismantleProcessSnapshot, with_dismantle_processes_from,
};
use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue, test_key,
};
use serde_json::{Value, json};

use super::{Route, TestResult};

pub(in super::super) const DATASET: &str = "/definitions/dismantleProcesses";

const RECORD: &str = "CraftingBlueprintRecord";
const BLUEPRINT: &str = "GenericCraftingBlueprint";
const PROCESS: &str = "GenericCraftingProcess_Dismantle";
const TIME: &str = "TimeValue_Partitioned";
const ROOT: &str = "libs/foundry/records/crafting/blueprints";
const GUID: u128 = 0xb100_0000_0000_4000_8000_0000_0000_0001;

pub(super) fn read() -> TestResult<Route> {
    let (_folder, archive) = archive(&database())?;
    let entry = DataForgeEntry::read(&archive)?;
    let built = build_catalog_from(
        &archive,
        &entry,
        CatalogOptions::new(UNIX_EPOCH).with_full_provenance(),
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?.with_dismantle_processes(
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| {
            verify_selected(&snapshot)?;
            let expected = catalog
                .pointer(DATASET)
                .ok_or("canonical dismantle process table")?;
            assert_eq!(
                &json!({"availability": snapshot.availability, "rows": snapshot.rows}),
                expected,
                "direct query matches the canonical row, including its original authored payload"
            );
            Route::queried_rows(
                DATASET,
                &snapshot.rows,
                "exact synthetic producer and scope parity available; installed producer parity open",
            )
        },
    )?
}

fn verify_selected(snapshot: &DismantleProcessSnapshot<'_>) -> TestResult<()> {
    let [row] = snapshot.rows.as_slice() else {
        return Err("one authored dismantle process".into());
    };
    assert_eq!(
        row.header().record_index().value(),
        Some(&0),
        "the row belongs to the requested physical owner"
    );
    let process = row
        .blueprint()
        .value()
        .and_then(|blueprint| blueprint.process_specific_data().value())
        .ok_or("the original process structure")?;
    let seconds = process.time_seconds();
    assert_eq!(
        seconds.value().map(|seconds| seconds.get()),
        Some(15),
        "conversion rounds the authored fractional seconds"
    );
    assert_eq!(
        seconds.origin().contains_encrypted_source(),
        Some(true),
        "conversion depends on the original encrypted DataForge Entry"
    );
    let time = process
        .dismantle_time()
        .value()
        .ok_or("the original time structure")?;
    let retained = serde_json::to_value(time.authored_fields())?;
    let values = retained
        .pointer("/values")
        .and_then(Value::as_array)
        .ok_or("authored time children")?;
    assert!(
        values.iter().any(|node| {
            node.pointer("/value/value") == Some(&json!({"type": "number", "value": {"type": "uint64", "value": "18446744073709551615"}}))
                && node.pointer("/value/containsEncryptedSource") == Some(&json!(true))
        }),
        "unknown owned numeric input survives with its original complete width and provenance"
    );
    Ok(())
}

#[test]
fn direct_dismantle_query_isolates_the_requested_physical_source() -> TestResult<()> {
    let broken_dismantle = TestRecord::new(
        GUID + 1,
        RECORD,
        "CraftingBlueprintRecord.BrokenDismantle",
        format!("{ROOT}/dismantle/brokendismantle.xml"),
    )
    .with_instance_index(u16::MAX);
    let broken_blueprint = TestRecord::new(
        GUID + 2,
        RECORD,
        "CraftingBlueprintRecord.BrokenCreation",
        format!("{ROOT}/crafting/brokencreation.xml"),
    )
    .with_instance_index(u16::MAX);
    let (_folder, archive) =
        archive(&database().record(broken_dismantle).record(broken_blueprint))?;
    let entry = DataForgeEntry::read(&archive)?;
    with_dismantle_processes_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.coverage.unreadable_bodies.as_slice(),
                &[] as &[u32],
                "a physical query does not read other candidates"
            );
            verify_selected(&snapshot)
        },
    )??;
    with_dismantle_processes_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        None,
        |snapshot| {
            assert_eq!(
                snapshot.coverage.unreadable_bodies.as_slice(),
                &[1_u32],
                "only a broken owner under the expected path contributes coverage"
            );
            verify_selected(&snapshot)
        },
    )?
}

fn database() -> DataForgeBuilder {
    let process = TestInstance::new(PROCESS)
        .value("efficiency", TestValue::Single(0.5))
        .value(
            "dismantleTime",
            TestValue::Struct(
                TestInstance::new(TIME)
                    .value("days", TestValue::Int32(0))
                    .value("hours", TestValue::Int32(0))
                    .value("minutes", TestValue::Int32(0))
                    .value("seconds", TestValue::Single(14.75))
                    .value("tickMask", TestValue::UInt64(u64::MAX)),
            ),
        );
    let blueprint =
        TestInstance::new(BLUEPRINT).value("processSpecificData", TestValue::Struct(process));
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(RECORD)
                .field("blueprint", TestType::StrongPointer(BLUEPRINT.to_owned())),
        )
        .structure(TestStruct::new(BLUEPRINT).field(
            "processSpecificData",
            TestType::StrongPointer(PROCESS.to_owned()),
        ))
        .structure(
            TestStruct::new(PROCESS)
                .field("efficiency", TestType::Single)
                .field("dismantleTime", TestType::Class(TIME.to_owned())),
        )
        .structure(
            TestStruct::new(TIME)
                .field("days", TestType::Int32)
                .field("hours", TestType::Int32)
                .field("minutes", TestType::Int32)
                .field("seconds", TestType::Single)
                .field("tickMask", TestType::UInt64),
        )
        .record(
            TestRecord::new(
                GUID,
                RECORD,
                "CraftingBlueprintRecord.FixtureProcess",
                format!("{ROOT}/dismantle/fixtureprocess.xml"),
            )
            .value("blueprint", TestValue::Struct(blueprint)),
        )
}

fn archive(database: &DataForgeBuilder) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("synthetic-dismantle.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted())
        .write_to(&path)?;
    Ok((directory, Archive::open_with_key(path, Some(test_key()))?))
}
