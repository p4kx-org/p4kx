// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::json;

use super::cargo_globals;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};

#[path = "../../tests/it/catalog_cargo_globals_fixture.rs"]
mod fixture;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn cargo_globals_keep_selected_values_and_markers_without_raw_members() -> TestResult {
    for layout in [
        fixture::Layout::Authored,
        fixture::Layout::DuplicateIdentity,
    ] {
        with_queries(layout, |parsed, scratch| {
            let staged = cargo_globals::collect(parsed, scratch, None)?;
            let loading = serde_json::to_value(staged.loading)?;
            let commodity = serde_json::to_value(staged.commodity)?;
            let rows = loading["rows"].as_array().ok_or("loading rows")?;
            assert_eq!(rows.len(), 2);
            assert_eq!(
                rows[0]["guid"] == rows[1]["guid"],
                matches!(layout, fixture::Layout::DuplicateIdentity)
            );
            assert!(rows.iter().any(|row| {
                row["recordPath"]["value"]
                    .as_str()
                    .is_some_and(|path| path.starts_with(fixture::LOADING_FILTER))
            }));
            assert_ne!(rows[0]["recordIndex"], rows[1]["recordIndex"]);
            for row in rows {
                let times = &row["autoLoadingBoxSizeLoadingTime"]["value"];
                assert!(row.get("additionalFields").is_none());
                assert!(times.get("additionalFields").is_none());
                assert_eq!(times["byScu"]["0.25"]["state"], "absent");
                assert!(times["byScu"]["0.25"]["value"].is_null());
                assert_eq!(
                    times["byScu"]["1"],
                    json!({
                        "containsEncryptedSource": true,
                        "state": "present",
                        "value": {"type": "f32", "finite": null, "nonFinite": "naN"}
                    })
                );
                assert_eq!(row["cargoDeckLoadingTimePerSCU"]["value"]["finite"], 0.25);
            }
            let shop = commodity.pointer("/rows/0").ok_or("commodity row")?;
            assert_eq!(shop["autoLoadingBasePrice"]["value"].as_i64(), Some(-300));
            let prices = &shop["autoLoadingBoxSizePrices"]["value"]["byScu"];
            assert_eq!(prices["0.125"]["value"], i32::MIN);
            assert_eq!(prices["32"]["value"], i32::MAX);
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn incomplete_selected_cargo_loading_fields_refuse_the_slice() -> TestResult {
    with_queries(fixture::Layout::InvalidTimeMember, |parsed, scratch| {
        assert!(matches!(
            cargo_globals::collect(parsed, scratch, None),
            Err(super::ReducedError::IncompleteScope(cargo_globals::LOADING))
        ));
        Ok(())
    })
}

fn with_queries(
    layout: fixture::Layout,
    check: impl FnOnce(
        &crate::catalog_queries::ParsedCatalogQueries<'_>,
        &std::path::Path,
    ) -> TestResult,
) -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", fixture::database(layout).build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    check(&session.parsed(None)?, scratch.path())
}
