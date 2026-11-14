// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::Archive;
use serde_json::Value;

use crate::catalog_contracts::{self, A, B, C, E, SUB_ONE};
use crate::contracts_fixture as fixture;
use crate::export_complete_pickup_source;
use crate::starmap_fixture;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn every_original_pickup_place_survives_without_changing_ranking_or_fallback() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        fixture::write_archive(directory.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let original = export_complete_pickup_source::locations(&archive)?;
    let expected: Vec<_> = (0..7)
        .map(|ordinal| starmap_fixture::guid_text(fixture::OBJ_STATION + ordinal))
        .collect();
    assert_eq!(
        original, expected,
        "the original seven physical references precede export"
    );
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        pickup_guids(catalog_contracts::entry(&catalog, B, None)?)?,
        original,
        "every original pickup place survives the catalog projection in authored order"
    );
    assert_selection_rules(&catalog)?;
    Ok(())
}

fn assert_selection_rules(catalog: &Value) -> TestResult {
    let expected: Vec<_> = [
        fixture::OBJ_SYSTEM,
        fixture::OBJ_PLANET,
        fixture::OBJ_STATION,
    ]
    .into_iter()
    .map(starmap_fixture::guid_text)
    .collect();
    assert_eq!(
        pickup_guids(catalog_contracts::entry(catalog, A, None)?)?,
        expected,
        "ranking, deduplication and omission of stars stay independent of list length"
    );
    let planet = vec![starmap_fixture::guid_text(fixture::OBJ_PLANET)];
    assert_eq!(
        pickup_guids(catalog_contracts::entry(catalog, A, Some(SUB_ONE))?)?,
        planet,
        "a sub-contract keeps its own location selection"
    );
    assert_eq!(
        pickup_guids(catalog_contracts::entry(catalog, E, None)?)?,
        planet,
        "an empty prerequisite selection still uses the actual broker location"
    );
    assert!(
        pickup_guids(catalog_contracts::entry(catalog, C, None)?)?.is_empty(),
        "an actual absence remains empty"
    );
    Ok(())
}

fn pickup_guids(row: &Value) -> Result<Vec<String>, Box<dyn Error>> {
    row.get("pickupPlaces")
        .and_then(Value::as_array)
        .ok_or_else(|| "pickupPlaces array".into())
        .and_then(|places| {
            places
                .iter()
                .map(|place| {
                    place
                        .get("starmapObjectGuid")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .ok_or_else(|| "pickup place GUID".into())
                })
                .collect()
        })
}
