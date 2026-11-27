// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::{ShipStorePledgeScope, ship_store_pledges};
use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::{Archive, Filter};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry};
use serde_json::{Value, json};

use super::super::{Route, SHIP_STORE_PLEDGES, TestResult};

pub(super) fn read() -> TestResult<Route> {
    let pledges = ship_store_pledges()?;
    let section = serde_json::to_value(&pledges)?;
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("curated-store-pledges.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            DataForgeBuilder::new().version(8).build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let filter = Filter::new("fixture-no-match");
    for selection in [None, Some(&filter)] {
        let options = CatalogOptions {
            filter: selection,
            ..CatalogOptions::new(UNIX_EPOCH)
        };
        let built = build_catalog(&archive, options.with_full_provenance())?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        assert_eq!(
            catalog.get("schemaVersion"),
            Some(&json!(3)),
            "the new section is additive within Catalog v3"
        );
        assert_eq!(
            catalog.pointer(SHIP_STORE_PLEDGES),
            Some(&section),
            "the direct SDK and the Normal producer share all curated rows and scope regardless of Filter"
        );
    }
    Route::queried_rows(SHIP_STORE_PLEDGES, std::slice::from_ref(&pledges), None)
}

#[test]
fn curated_scope_and_source_are_part_of_the_catalog_contract() -> TestResult {
    let pledges = ship_store_pledges()?;
    assert_eq!(
        pledges.snapshot().scope(),
        ShipStorePledgeScope::ReviewedCandidates,
        "the snapshot retains its reviewed candidate scope"
    );
    let validator = jsonschema::validator_for(
        &schemars::schema_for!(p4kx_export::catalog_queries::equipment::ShipStorePledges)
            .to_value(),
    )?;
    let encoded = serde_json::to_value(&pledges)?;
    assert!(
        validator.is_valid(&encoded),
        "the exact bundled section satisfies the schema"
    );
    for (path, replacement) in [
        ("/snapshot/source", json!("unspecified")),
        ("/snapshot/sourceRevision", json!("old")),
        ("/snapshot/scope", json!("currentPopulation")),
        ("/snapshot/revision", json!(0)),
    ] {
        let mut candidate = encoded.clone();
        *candidate
            .pointer_mut(path)
            .ok_or("the curated schema address")? = replacement;
        assert!(
            !validator.is_valid(&candidate),
            "refuse a wrong schema value at {path}"
        );
    }
    let mut future = encoded;
    *future
        .pointer_mut("/rows/0/productionStatus")
        .ok_or("the curated production status address")? = json!("future-status");
    assert!(
        validator.is_valid(&future),
        "consumers of this schema can accept new published status strings"
    );
    assert_eq!(
        p4kx_export::catalog_schema()
            .pointer("/$defs/ShipStorePledgeSnapshot/properties/captureDate/format"),
        Some(&json!("date")),
        "the source capture date has a formal date format"
    );
    Ok(())
}
