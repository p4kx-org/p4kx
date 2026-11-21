// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    MiningDefinitionSnapshot,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{DataForgeEntry, FieldOrigin, InputScope, SourceInventory};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};

use super::super::{
    HARVESTABLE_PROVIDER_PRESETS, MINEABLE_COMPOSITIONS, MINEABLE_ELEMENTS, MINING_GLOBALS,
    REFINING_PROCESSES,
};
use super::{Route, TestResult};

#[path = "sdk_mining_family_fixture.rs"]
mod fixture;

pub(super) fn read() -> TestResult<[Route; 5]> {
    let (_folder, archive) = fixture::open()?;
    let entry = DataForgeEntry::read(&archive)?;
    let mut settings = CatalogOptions::new(UNIX_EPOCH);
    settings.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog_from(&archive, &entry, &settings)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    let selection = CatalogRecordSelection::All;
    let elements = parsed.with_mineable_elements(selection, None, |snapshot| {
        array(MINEABLE_ELEMENTS, &snapshot, &catalog, |row| {
            row.instability.origin()
        })
    })??;
    let compositions = parsed.with_mineable_compositions(selection, None, |snapshot| {
        array(MINEABLE_COMPOSITIONS, &snapshot, &catalog, |row| {
            row.minimum_distinct_elements.origin()
        })
    })??;
    let globals = parsed.with_mining_global_params(selection, None, |snapshot| {
        table(MINING_GLOBALS, &snapshot, &catalog, |row| {
            row.composition_mass_multiplier().origin()
        })
    })??;
    let refining = parsed.with_refining_processes(selection, None, |snapshot| {
        table(REFINING_PROCESSES, &snapshot, &catalog, |row| {
            row.refining_speed().origin()
        })
    })??;
    let providers = parsed.read_harvestable_provider_presets(selection, None)?;
    let providers = witness(
        HARVESTABLE_PROVIDER_PRESETS,
        &providers.rows,
        None,
        &catalog,
        &providers.source_inventory,
        |row| row.groups.origin(),
    )?;
    Ok([elements, compositions, globals, refining, providers])
}

fn array<Row: JsonSchema + Serialize>(
    dataset: &'static str,
    snapshot: &MiningDefinitionSnapshot<'_, Row>,
    catalog: &Value,
    origin: impl FnOnce(&Row) -> &FieldOrigin,
) -> TestResult<Route> {
    witness(
        dataset,
        snapshot.rows(),
        None,
        catalog,
        snapshot.source_inventory(),
        origin,
    )
}

fn table<Row: JsonSchema + Serialize>(
    dataset: &'static str,
    snapshot: &MiningDefinitionSnapshot<'_, Row>,
    catalog: &Value,
    origin: impl FnOnce(&Row) -> &FieldOrigin,
) -> TestResult<Route> {
    witness(
        dataset,
        snapshot.rows(),
        Some(snapshot.availability()),
        catalog,
        snapshot.source_inventory(),
        origin,
    )
}

fn witness<Row: JsonSchema + Serialize>(
    dataset: &'static str,
    rows: &[Row],
    availability: Option<DatasetAvailability>,
    catalog: &Value,
    inventory: &SourceInventory,
    origin: impl FnOnce(&Row) -> &FieldOrigin,
) -> TestResult<Route> {
    let [row] = rows else {
        return Err(format!("{dataset} must select exactly one authored fixture").into());
    };
    let expected = catalog.pointer(dataset).ok_or("canonical mining dataset")?;
    let value = match availability {
        Some(availability) => json!({"availability": availability, "rows": rows}),
        None => serde_json::to_value(rows)?,
    };
    assert_eq!(
        &value, expected,
        "canonical values and original source references at {dataset}"
    );
    let origin = origin(row);
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "original {dataset} field came from the encrypted DataForge Entry"
    );
    assert!(
        origin.source_set_complete()
            && inventory.iter_uses().any(|used| {
                used.scope() == InputScope::Payload && origin.source_uses_refs().contains(used.id())
            }),
        "original {dataset} payload evidence is present in the selective result"
    );
    Route::queried_rows(
        dataset,
        rows,
        "encrypted synthetic producer parity available; installed producer parity open",
    )
}
