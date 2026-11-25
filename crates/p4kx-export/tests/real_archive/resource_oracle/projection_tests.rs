// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, ArchiveWatch, DataForgeEntry};
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value;

use super::super::resource_projection;
use super::fixture::complete_sources;
use super::{crafting_fixture, groups};
use crate::{CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from};

const REWARD_POOL: u128 = 0x4700;
const REWARD_SOURCE: u128 = 0x4701;
const REWARD_PATH: &str = "libs/foundry/records/fixtures/resource_reward.xml";

#[test]
fn resource_view_matches_wire_tables_and_original_owners() -> Result<(), Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let path = crafting_fixture::write_archive(folder.path())?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let (view, health) = resource_projection::read(&archive, &entry, UNIX_EPOCH)?;
    complete_sources(&health)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let full = build_catalog_from(&archive, &entry, options.with_full_provenance())?;
    let document: Value = serde_json::from_slice(full.json())?;
    compare_regions(&view, &document);
    assert!(
        health.kind(Kind::Ships).is_none(),
        "the oracle only accounts for the resource domain and its source dependencies"
    );
    compare_fallback(&view)?;
    let forge = entry.open()?;
    let archive_watch = ArchiveWatch::new(&archive);
    assert_eq!(
        groups::check(&forge, entry.origin(), &view, &archive_watch)?,
        groups::check(&forge, entry.origin(), &document, &archive_watch)?,
        "the scoped group output retains physical ownership and locale reference closure"
    );
    Ok(())
}

#[test]
fn unrelated_reward_source_scan_leaves_the_scoped_resource_regions_equal()
-> Result<(), Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let database = crafting_fixture::database()
        .structure(TestStruct::new("BlueprintPoolRecord"))
        .structure(
            TestStruct::new("FixtureRewardSource")
                .field(
                    "pool",
                    TestType::ReferenceTo("BlueprintPoolRecord".to_owned()),
                )
                .field("chance", TestType::Single),
        )
        .record(TestRecord::new(
            REWARD_POOL,
            "BlueprintPoolRecord",
            "BlueprintPoolRecord.FixtureResource",
            "libs/foundry/records/crafting/blueprintrewards/fixture_resource.xml",
        ))
        .record(
            TestRecord::new(
                REWARD_SOURCE,
                "FixtureRewardSource",
                "FixtureRewardSource.Resource",
                REWARD_PATH,
            )
            .value("pool", TestValue::Reference(REWARD_POOL))
            .value("chance", TestValue::Single(0.5)),
        );
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database.build()).encrypted())
        .entry(crafting_fixture::english())
        .entry(crafting_fixture::german())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let full = build_catalog_from(&archive, &entry, options.with_full_provenance())?;
    let document: Value = serde_json::from_slice(full.json())?;
    assert_eq!(
        document
            .pointer("/declarations/blueprintRewardSources/rows/0/recordPath/value")
            .and_then(Value::as_str),
        Some(REWARD_PATH.trim_end_matches(".xml")),
        "the complete catalog finds the authored reward"
    );
    let (view, health) = resource_projection::read(&archive, &entry, UNIX_EPOCH)?;
    complete_sources(&health)?;
    compare_regions(&view, &document);
    compare_fallback(&view)?;
    Ok(())
}

pub(super) fn compare_regions(scoped: &Value, full: &Value) {
    for path in [
        "/schemaVersion",
        "/source",
        "/definitions/commodities",
        "/definitions/resourceTypes",
        "/definitions/resourceGroups",
        "/declarations/resourceGroupMembers",
        "/projections/commodityPresentation",
        "/projections/resourceGroupParents",
    ] {
        assert!(
            scoped.pointer(path).is_some(),
            "missing scoped region {path}"
        );
        assert_eq!(
            scoped.pointer(path),
            full.pointer(path),
            "scoped resource region differs at {path}"
        );
    }
}

fn compare_fallback(view: &Value) -> Result<(), Box<dyn Error>> {
    let iron = format!("{:032x}", crafting_fixture::RESOURCE_IRON);
    let rows = view
        .pointer("/definitions/commodities/rows")
        .and_then(Value::as_array)
        .ok_or("scoped Commodity rows")?;
    assert!(
        rows.iter().any(|row| {
            row.pointer("/guid/value")
                .and_then(Value::as_str)
                .is_some_and(|guid| guid.replace('-', "") == iron)
                && row.pointer("/selection/value").and_then(Value::as_str)
                    == Some("resourceFallback")
        }),
        "the Blueprint cost backfill survives the scoped scan"
    );
    Ok(())
}
