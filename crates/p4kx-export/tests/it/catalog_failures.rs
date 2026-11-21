// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Explicit diagnostic publication retains readable rows and measured source failures.

use std::error::Error;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{BuiltCatalog, CatalogOptions, Kind, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const GOOD: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0001;
const BROKEN_B: u128 = 0x9000_0000_0000_4000_8000_0000_0000_0002;
const BROKEN_A: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0003;
const BROKEN_PATH: &str = "libs/foundry/records/crafting/blueprints/crafting/broken_b";

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("SCItemLocalization")
                .field("Name", TestType::Locale)
                .field("Description", TestType::Locale),
        )
        .structure(
            TestStruct::new("SCItemManufacturer")
                .field("Code", TestType::String)
                .field(
                    "Localization",
                    TestType::Class("SCItemLocalization".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("CraftingBlueprintRecord")
                .field("a", TestType::Int8)
                .declared_size(3),
        )
        .record(
            TestRecord::new(
                GOOD,
                "SCItemManufacturer",
                "SCItemManufacturer.ACAS",
                "libs/foundry/records/scitemmanufacturer/acas.xml",
            )
            .value("Code", TestValue::text("ACAS"))
            .value(
                "Localization",
                TestValue::Struct(TestInstance::new("SCItemLocalization")),
            ),
        )
        .record(broken(BROKEN_B, "broken_b"))
        .record(broken(BROKEN_A, "broken_a"))
}

fn broken(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "CraftingBlueprintRecord",
        format!("CraftingBlueprintRecord.{name}"),
        format!("libs/foundry/records/crafting/blueprints/crafting/{name}.xml"),
    )
}

fn built() -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path: PathBuf = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    let options = CatalogOptions {
        publication_mode: p4kx_export::CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000))
    };
    let built = build_catalog(&archive, &options)?;
    let catalog = serde_json::from_slice(built.json())?;
    Ok((built, catalog))
}

#[test]
fn an_unreadable_record_is_left_out_and_the_catalog_is_still_built() -> TestResult {
    let (built, catalog) = built()?;

    assert_eq!(
        catalog["definitions"]["manufacturers"]["rows"]
            .as_array()
            .map(Vec::len),
        Some(1),
        "the readable manufacturer is written"
    );
    assert_eq!(
        catalog["definitions"]["blueprints"]["rows"]
            .as_array()
            .map(Vec::len),
        Some(0),
        "the unreadable blueprints are left out"
    );
    let blueprints = built.summary().kind(Kind::Blueprints).ok_or("blueprints")?;
    assert_eq!(blueprints.count, 0, "none written");
    assert_eq!(blueprints.skipped, 0, "failures are not leftovers");
    Ok(())
}

#[test]
fn an_unreadable_record_is_counted_and_reported_with_path_guid_and_reason() -> TestResult {
    let (built, _) = built()?;
    let summary = built.summary();

    assert!(!summary.is_complete(), "a Record failed");
    let guids: Vec<String> = summary
        .failures()
        .iter()
        .map(|failure| failure.guid().to_string())
        .collect();
    assert_eq!(
        guids,
        [
            "20000000-0000-4000-8000-000000000003",
            "90000000-0000-4000-8000-000000000002"
        ],
        "ordered by GUID"
    );
    let failure = summary
        .failures()
        .iter()
        .find(|failure| failure.record_path() == BROKEN_PATH)
        .ok_or("the record path")?;
    assert!(!failure.reason().is_empty(), "a reason");
    Ok(())
}

#[test]
fn a_catalog_without_unreadable_records_is_complete() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let forge = DataForgeBuilder::new().version(8);
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", forge.build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;

    let built = build_catalog(
        &archive,
        &CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000)),
    )?;

    assert!(built.summary().is_complete(), "no failure");
    assert!(built.summary().failures().is_empty(), "none listed");
    Ok(())
}
