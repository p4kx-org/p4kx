// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selection rules and completeness of the Basetool catalog, through the public API.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{
    BuiltCatalog, CancelToken, CatalogError, CatalogOptions, CatalogProgress, EXCLUDED_TOKENS,
    Filter, Kind, SizeWarning, build_catalog,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue, write_build_manifest,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const EXPORTED_AT: Duration = Duration::from_secs(1_792_000_000);

const MANUFACTURERS: [(u128, &str, &str, &str); 8] = [
    (
        0xf000_0000_0000_4000_8000_0000_0000_0001,
        "RSI",
        "scitemmanufacturer/scitemmanufacturer.rsi",
        "RSI",
    ),
    (
        0x1000_0000_0000_4000_8000_0000_0000_0002,
        "ACAS",
        "scitemmanufacturer/acas",
        "ACAS",
    ),
    (
        0x8000_0000_0000_4000_8000_0000_0000_0003,
        "Paint_Logo",
        "scitemmanufacturer/paintcolorlogos/paint_logo",
        "",
    ),
    (
        0x2000_0000_0000_4000_8000_0000_0000_0004,
        "Contestant",
        "scitemmanufacturer/contestant",
        "CTST",
    ),
    (
        0x3000_0000_0000_4000_8000_0000_0000_0005,
        "Paint_Test_01",
        "scitemmanufacturer/paint_test_01",
        "",
    ),
    (
        0x4000_0000_0000_4000_8000_0000_0000_0006,
        "Base",
        "scitemmanufacturer/templates/manufacturer_template",
        "",
    ),
    (
        0x5000_0000_0000_4000_8000_0000_0000_0007,
        "DUMMY",
        "scitemmanufacturer/placeholder_corp",
        "",
    ),
    (
        0x6000_0000_0000_4000_8000_0000_0000_0008,
        "Elsewhere",
        "scitemmanufacturers_old/elsewhere",
        "ELSE",
    ),
];

fn database() -> DataForgeBuilder {
    let builder = DataForgeBuilder::new()
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
        .structure(TestStruct::new("ShopFranchise").field("Code", TestType::String))
        .record(
            TestRecord::new(
                0x7000_0000_0000_4000_8000_0000_0000_0009,
                "ShopFranchise",
                "ShopFranchise.Shop",
                "libs/foundry/records/scitemmanufacturer/shop/shop.xml",
            )
            .value("Code", TestValue::text("SHOP")),
        );
    MANUFACTURERS
        .into_iter()
        .fold(builder, |builder, (guid, class, path, code)| {
            builder.record(
                TestRecord::new(
                    guid,
                    "SCItemManufacturer",
                    format!("SCItemManufacturer.{class}"),
                    format!("libs/foundry/records/{path}.xml"),
                )
                .value("Code", TestValue::text(code))
                .value("Localization", localization(class, code)),
            )
        })
}

fn localization(class: &str, code: &str) -> TestValue {
    let key = |kind: &str| {
        if code.is_empty() {
            TestValue::text("")
        } else {
            TestValue::text(format!("@manufacturer_{kind}{class}"))
        }
    };
    TestValue::Struct(
        TestInstance::new("SCItemLocalization")
            .value("Name", key("Name"))
            .value("Description", key("Desc")),
    )
}

fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let channel = dir.join("PTU");
    std::fs::create_dir(&channel)?;
    write_build_manifest(
        &channel,
        "sc-alpha-4.4.0",
        "4.4.162.12345",
        "10012345",
        "0123456789abcdef",
    )?;
    let path = channel.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()).encrypted())
        .write_to(&path)?;
    Ok(path)
}

fn catalog_with(options: &CatalogOptions<'_>) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let built = build_catalog(&archive, options)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

fn options() -> CatalogOptions<'static> {
    CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT)
}

fn class_names(catalog: &Value) -> Vec<&str> {
    catalog["manufacturers"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|entry| entry["className"].as_str())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn manufacturers_are_selected_by_record_path_prefix_and_type_ordered_by_guid() -> TestResult {
    let (_, catalog) = catalog_with(&options())?;
    assert_eq!(
        class_names(&catalog),
        ["ACAS", "Contestant", "Paint_Logo", "RSI"],
        "under the prefix, of type SCItemManufacturer, without leftovers, by GUID"
    );
    Ok(())
}

#[test]
fn a_manufacturer_carries_guid_class_name_record_path_code_name_and_description() -> TestResult {
    let (_, catalog) = catalog_with(&options())?;
    assert_eq!(
        catalog["manufacturers"][0],
        serde_json::json!({
            "guid": "10000000-0000-4000-8000-000000000002",
            "className": "ACAS",
            "recordPath": "libs/foundry/records/scitemmanufacturer/acas",
            "code": "ACAS",
            "name": {"key": "@manufacturer_NameACAS", "en": null, "de": null},
            "description": {"key": "@manufacturer_DescACAS", "en": null, "de": null},
        }),
        "the manufacturer fields"
    );
    Ok(())
}

#[test]
fn development_leftovers_are_excluded_by_token_and_counted_as_skipped() -> TestResult {
    assert_eq!(
        EXCLUDED_TOKENS,
        ["test", "template", "dummy", "debug", "deprecated", "dev"],
        "documented list"
    );
    let (built, catalog) = catalog_with(&options())?;
    let names = class_names(&catalog);
    for leftover in ["Paint_Test_01", "Base", "DUMMY"] {
        assert!(!names.contains(&leftover), "{leftover} is excluded");
    }
    assert!(names.contains(&"Contestant"), "a token, not a substring");
    let kind = built.summary().kind(Kind::Manufacturers);
    assert_eq!(
        kind.map(|kind| (kind.count, kind.skipped)),
        Some((4, 3)),
        "counts"
    );
    Ok(())
}

#[test]
fn without_a_filter_every_kind_is_complete() -> TestResult {
    let (built, catalog) = catalog_with(&options())?;
    assert_eq!(
        catalog["completeness"]["manufacturers"],
        serde_json::json!({"status": "complete"}),
        "complete"
    );
    assert!(
        built
            .summary()
            .kind(Kind::Manufacturers)
            .is_some_and(|kind| kind.complete),
        "summary says complete"
    );
    Ok(())
}

#[test]
fn a_filter_restricts_the_selection_and_marks_the_kind_partial() -> TestResult {
    let filter = Filter::new("*/paint*");
    let (built, catalog) = catalog_with(&CatalogOptions {
        filter: Some(&filter),
        ..options()
    })?;
    assert_eq!(
        class_names(&catalog),
        ["Paint_Logo"],
        "only the filtered Records"
    );
    assert_eq!(
        catalog["completeness"]["manufacturers"],
        serde_json::json!({"status": "partial", "filter": "*/paint*"}),
        "partial with the filter"
    );
    let kind = built.summary().kind(Kind::Manufacturers);
    assert_eq!(
        kind.map(|kind| (kind.count, kind.skipped, kind.complete)),
        Some((1, 1, false)),
        "the filtered leftover is skipped"
    );
    Ok(())
}

#[test]
fn the_source_names_channel_build_archive_tool_and_export_time() -> TestResult {
    let (built, catalog) = catalog_with(&options())?;
    assert_eq!(catalog["schemaVersion"], p4kx_export::SCHEMA_VERSION, "schema version");
    let source = &catalog["source"];
    assert_eq!(source["channel"], "PTU", "channel");
    assert_eq!(
        source["build"],
        serde_json::json!({
            "branch": "sc-alpha-4.4.0",
            "version": "4.4.162.12345",
            "changeNumber": "10012345",
            "buildId": "0123456789abcdef",
            "publicPatch": null,
        }),
        "build"
    );
    assert!(
        source["archiveSize"].as_u64().is_some_and(|size| size > 0),
        "size"
    );
    assert!(source["archiveModified"].is_string(), "timestamp");
    assert_eq!(
        source["toolVersion"],
        env!("CARGO_PKG_VERSION"),
        "tool version"
    );
    assert_eq!(source["exportedAt"], "2026-10-14T17:46:40Z", "export time");
    assert_eq!(
        built.summary().size(),
        u64::try_from(built.json().len())?,
        "the summary gives the size"
    );
    Ok(())
}

#[test]
fn two_runs_write_identical_bytes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let first = build_catalog(&archive, &options())?;
    let second = build_catalog(&archive, &options())?;
    assert!(first.json() == second.json(), "byte-identical");
    assert!(
        first.json().ends_with(b"}\n"),
        "one JSON document and a newline"
    );
    Ok(())
}

#[test]
fn progress_reaches_every_record_and_cancellation_stops_the_build() -> TestResult {
    let last = std::sync::Mutex::new(CatalogProgress::default());
    let report = |progress: CatalogProgress| {
        if let Ok(mut last) = last.lock() {
            *last = (*last).max(progress);
        }
    };
    catalog_with(&CatalogOptions {
        progress: Some(&report),
        ..options()
    })?;
    let seen = *last.lock().map_err(|_| "poisoned")?;
    assert_eq!(seen.records_done, 9, "every Record scanned");
    assert_eq!(seen.records_total, 9, "total");

    let cancel = CancelToken::new();
    cancel.cancel();
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let result = build_catalog(
        &archive,
        &CatalogOptions {
            cancel: Some(&cancel),
            ..options()
        },
    );
    assert!(matches!(result, Err(CatalogError::Cancelled)), "cancelled");
    Ok(())
}

#[test]
fn an_archive_without_dataforge_is_an_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data\\readme.txt",
            b"no database".to_vec(),
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    let result = build_catalog(&archive, &options());
    assert!(matches!(result, Err(CatalogError::Load(_))), "load error");
    Ok(())
}

#[test]
fn the_size_warning_comes_before_the_64_mb_upload_limit() {
    let limit = 64_000_000;
    assert_eq!(SizeWarning::for_size(1_000_000), None, "small");
    assert_eq!(SizeWarning::for_size(57_599_999), None, "below 90 %");
    assert_eq!(
        SizeWarning::for_size(57_600_000),
        Some(SizeWarning::NearLimit),
        "from 90 %"
    );
    assert_eq!(
        SizeWarning::for_size(limit),
        Some(SizeWarning::NearLimit),
        "at the limit"
    );
    assert_eq!(
        SizeWarning::for_size(limit + 1),
        Some(SizeWarning::OverLimit),
        "over the limit"
    );
}

#[test]
fn an_empty_localisation_key_is_null() -> TestResult {
    let (_, catalog) = catalog_with(&options())?;
    let paint = catalog["manufacturers"]
        .as_array()
        .and_then(|list| list.iter().find(|m| m["className"] == "Paint_Logo"))
        .ok_or("Paint_Logo")?;
    assert_eq!(paint["name"]["key"], Value::Null, "no key");
    assert_eq!(paint["description"]["key"], Value::Null, "no key");
    Ok(())
}
