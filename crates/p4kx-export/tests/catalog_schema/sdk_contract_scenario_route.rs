// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::contract_scenarios::with_contract_scenarios_from;
use p4kx_export::{CatalogOptions, Filter, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, SourceInventory};
use serde_json::Value;

use super::{Route, TestResult};

use super::super::reputation_fixture as fixture;

pub(super) const DATASET: &str = "/contractScenarios";

pub(super) fn read() -> TestResult<Route> {
    let folder = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        fixture::write_archive(folder.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut records = forge.records().filter(|record| {
        record
            .type_name()
            .is_ok_and(|name| name == "ScenarioProgress")
    });
    let record = records.next().ok_or("physical scenario fixture")?;
    assert!(
        records.next().is_none(),
        "the synthetic producer selects one physical scenario"
    );
    let filter = Filter::new("contractscenarios");
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.filter = Some(&filter);
    let catalog: Value =
        serde_json::from_slice(build_catalog_from(&archive, &entry, &options)?.json())?;
    with_contract_scenarios_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(record.index()),
        None,
        |snapshot| -> TestResult<Route> {
            let rows: Value = serde_json::from_slice(&serde_json::to_vec(&snapshot.rows)?)?;
            assert_eq!(
                catalog.pointer(DATASET),
                Some(&rows),
                "the physical query matches the normal selected producer"
            );
            assert!(
                snapshot.coverage.unreadable_headers.is_empty()
                    && snapshot.coverage.unreadable_bodies.is_empty()
                    && snapshot.coverage.incomplete_fields.is_empty(),
                "irrelevant authored source data cannot degrade the scope"
            );
            check_reputation_uses(&catalog, &snapshot.source_inventory)?;
            Route::queried_rows(
                DATASET,
                &snapshot.rows,
                "selected ScenarioProgress rows and source parity verified on a synthetic producer; installed producer parity, enumeration and memory open",
            )
        },
    )?
}

fn check_reputation_uses(catalog: &Value, inventory: &SourceInventory) -> TestResult<()> {
    let path = "/contractScenarios/0/factionRewardTiers/value/0/value/factionReputation/value/guid/sourceUsesRefs";
    let ids = catalog
        .pointer(path)
        .and_then(Value::as_array)
        .ok_or("original reputation field uses")?;
    let catalog_uses = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("producer use inventory")?;
    assert!(
        !ids.is_empty(),
        "the selected Faction field has an original use"
    );
    for id in ids {
        let id = id.as_str().ok_or("actual source use ID")?;
        let sdk = inventory
            .iter_uses()
            .find(|used| used.id().as_str() == id)
            .ok_or("same physical use exists in the SDK inventory")?;
        let producer = catalog_uses
            .iter()
            .find(|used| used.get("id").and_then(Value::as_str) == Some(id))
            .ok_or("same physical use exists in the producer inventory")?;
        assert_eq!(
            serde_json::to_value(sdk)?,
            *producer,
            "record ordinal and source-use evidence agree"
        );
    }
    Ok(())
}
