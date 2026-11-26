// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::{Value, json};

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
            let rows = loading
                .get("rows")
                .and_then(Value::as_array)
                .ok_or("loading rows")?;
            let [first, second] = rows.as_slice() else {
                return Err("two physical loading rows".into());
            };
            assert_eq!(
                first.pointer("/guid/value") == second.pointer("/guid/value"),
                matches!(layout, fixture::Layout::DuplicateIdentity)
            );
            assert!(rows.iter().any(|row| {
                row.pointer("/recordPath/value")
                    .and_then(Value::as_str)
                    .is_some_and(|path| path.starts_with(fixture::LOADING_FILTER))
            }));
            assert_ne!(
                first.pointer("/recordIndex/value"),
                second.pointer("/recordIndex/value")
            );
            for row in rows {
                let times = row
                    .pointer("/autoLoadingBoxSizeLoadingTime/value")
                    .ok_or("selected loading times")?;
                assert!(row.get("additionalFields").is_none());
                assert!(times.get("additionalFields").is_none());
                assert_eq!(
                    times.pointer("/byScu/0.25/state").and_then(Value::as_str),
                    Some("absent")
                );
                assert_eq!(times.pointer("/byScu/0.25/value"), Some(&Value::Null));
                assert_eq!(
                    times.pointer("/byScu/1"),
                    Some(&json!({
                        "containsEncryptedSource": true,
                        "state": "present",
                        "value": {"type": "f32", "finite": null, "nonFinite": "naN"}
                    }))
                );
                assert_eq!(
                    row.pointer("/cargoDeckLoadingTimePerSCU/value/finite")
                        .and_then(Value::as_f64),
                    Some(0.25)
                );
            }
            let shop = commodity.pointer("/rows/0").ok_or("commodity row")?;
            assert_eq!(
                shop.pointer("/autoLoadingBasePrice/value")
                    .and_then(Value::as_i64),
                Some(-300)
            );
            let prices = shop
                .pointer("/autoLoadingBoxSizePrices/value/byScu")
                .ok_or("selected prices")?;
            assert_eq!(
                prices.pointer("/0.125/value").and_then(Value::as_i64),
                Some(i64::from(i32::MIN))
            );
            assert_eq!(
                prices.pointer("/32/value").and_then(Value::as_i64),
                Some(i64::from(i32::MAX))
            );
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
