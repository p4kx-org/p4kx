// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selection rules and completeness of the Basetool catalog, through the public API.

use std::error::Error;

use p4kx_export::{
    CancelToken, CatalogError, CatalogOptions, CatalogProgress, EXCLUDED_TOKENS, Filter, Kind,
    build_catalog,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

#[path = "catalog_selection_fixture.rs"]
mod fixture;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn manufacturers_are_selected_by_record_path_prefix_and_type_ordered_by_guid() -> TestResult {
    let (_, catalog) = fixture::catalog_with(&fixture::options())?;
    assert_eq!(
        fixture::class_names(&catalog)?,
        ["ACAS", "Contestant", "Paint_Logo", "RSI"],
        "under the prefix, of type SCItemManufacturer, without leftovers, by GUID"
    );
    Ok(())
}

#[test]
fn a_manufacturer_carries_guid_class_name_record_path_code_name_and_description() -> TestResult {
    let (_, catalog) = fixture::catalog_with(&fixture::options())?;
    assert!(catalog.get("manufacturers").is_none(), "no legacy array");
    let row = &catalog["definitions"]["manufacturers"]["rows"][0];
    assert_eq!(row["guid"]["value"], "10000000-0000-4000-8000-000000000002");
    assert_eq!(row["className"]["value"], "ACAS");
    assert_eq!(
        row["recordPath"]["value"],
        "libs/foundry/records/scitemmanufacturer/acas"
    );
    assert_eq!(row["code"]["value"], "ACAS");
    let localization = &row["localization"]["value"];
    assert_eq!(row["localization"]["state"], "present");
    assert_eq!(localization["sourceType"]["value"], "SCItemLocalization");
    for (field, domain, key) in [
        ("name", "publicNames", "@manufacturer_NameACAS"),
        ("description", "gameTexts", "@manufacturer_DescACAS"),
    ] {
        let id = &localization[field]["value"];
        let text = catalog["texts"][domain]
            .as_array()
            .ok_or("text table")?
            .iter()
            .find(|text| text["id"]["value"] == *id)
            .ok_or("canonical text")?;
        assert_eq!(text["key"]["value"], key);
    }
    Ok(())
}

#[test]
fn development_leftovers_are_excluded_by_token_and_counted_as_skipped() -> TestResult {
    assert_eq!(
        EXCLUDED_TOKENS,
        ["test", "template", "dummy", "debug", "deprecated", "dev"],
        "documented list"
    );
    let (built, catalog) = fixture::catalog_with(&fixture::options())?;
    let names = fixture::class_names(&catalog)?;
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
    let (built, catalog) = fixture::catalog_with(&fixture::options())?;
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
    let (built, catalog) = fixture::catalog_with(&CatalogOptions {
        filter: Some(&filter),
        ..fixture::options()
    })?;
    assert_eq!(
        fixture::class_names(&catalog)?,
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
    let (built, catalog) = fixture::catalog_with(&fixture::options())?;
    assert_eq!(
        catalog["schemaVersion"],
        p4kx_export::SCHEMA_VERSION,
        "schema version"
    );
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
    assert_eq!(source["archiveKey"], "given", "key");
    assert_eq!(source["encryptedEntriesUnread"], 0, "nothing unread");
    assert_eq!(built.summary().archive_key().entries_unread(), 0, "summary");
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
    let archive = Archive::open_with_key(
        fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let first = build_catalog(&archive, &fixture::options())?;
    let second = build_catalog(&archive, &fixture::options())?;
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
    fixture::catalog_with(&CatalogOptions {
        progress: Some(&report),
        ..fixture::options()
    })?;
    let seen = *last.lock().map_err(|_| "poisoned")?;
    assert_eq!(seen.records_done, 9, "every Record scanned");
    assert_eq!(seen.records_total, 9, "total");

    let cancel = CancelToken::new();
    cancel.cancel();
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let result = build_catalog(
        &archive,
        &CatalogOptions {
            cancel: Some(&cancel),
            ..fixture::options()
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
    let result = build_catalog(&archive, &fixture::options());
    assert!(matches!(result, Err(CatalogError::Load(_))), "load error");
    Ok(())
}

#[test]
fn an_empty_localisation_key_is_retained_in_its_canonical_text_domain() -> TestResult {
    let (_, catalog) = fixture::catalog_with(&fixture::options())?;
    let paint = catalog["definitions"]["manufacturers"]["rows"]
        .as_array()
        .and_then(|list| {
            list.iter()
                .find(|m| m["className"]["value"] == "Paint_Logo")
        })
        .ok_or("Paint_Logo")?;
    for (field, domain) in [("name", "publicNames"), ("description", "gameTexts")] {
        let reference = &paint["localization"]["value"][field];
        assert_eq!(reference["state"], "present");
        let text = catalog["texts"][domain]
            .as_array()
            .ok_or("text table")?
            .iter()
            .find(|text| text["id"]["value"] == reference["value"])
            .ok_or("retained empty key")?;
        assert_eq!(text["key"]["state"], "present");
        assert_eq!(text["key"]["value"], "");
    }
    Ok(())
}
