// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::{CatalogLanguageSelection, ReputationRows, ReputationSelection};
use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, FieldOrigin, InputScope, SourceInventory};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};

use super::super::{FACTION_REPUTATIONS, REPUTATION_SCOPES, REPUTATION_STANDINGS};
use super::{Route, TestResult, reputation_fixture};

pub(super) fn read() -> TestResult<[Route; 3]> {
    let folder = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        reputation_fixture::write_archive(folder.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let entry = DataForgeEntry::read(&archive)?;
    let built = build_catalog_from(
        &archive,
        &entry,
        CatalogOptions::new(UNIX_EPOCH).with_full_provenance(),
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    full_proof::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?
    .with_reputation(ReputationSelection::all(), None, |snapshot| {
        let factions = snapshot
            .faction_reputations
            .ok_or("faction reputation result")?;
        let scopes = snapshot.scopes.ok_or("scope result")?;
        let standings = snapshot.standings.ok_or("standing result")?;
        let [faction, ..] = factions.rows.as_slice() else {
            return Err("the selected faction reputation fixture".into());
        };
        let [scope] = scopes.rows.as_slice() else {
            return Err("the selected scope fixture".into());
        };
        let [standing, ..] = standings.rows.as_slice() else {
            return Err("the selected standing fixture".into());
        };
        let slots = scope.standings().ok_or("ordered original standing slots")?;
        let relations = faction.relations();
        let hostile = relations
            .hostility_params()
            .value()
            .ok_or("original hostile standing rule")?;
        for origin in [
            faction.display_name().origin(),
            relations.allies().origin(),
            hostile.scope().origin(),
            hostile.standing().origin(),
            slots.origin(),
            standing.min_reputation().origin(),
        ] {
            check_original_origin(origin, &snapshot.source_inventory);
        }
        Ok([
            witness(FACTION_REPUTATIONS, &factions, &catalog)?,
            witness(REPUTATION_SCOPES, &scopes, &catalog)?,
            witness(REPUTATION_STANDINGS, &standings, &catalog)?,
        ])
    })?
}

fn check_original_origin(origin: &FieldOrigin, inventory: &SourceInventory) {
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "the original reputation field comes from the encrypted DataForge Entry"
    );
    assert!(
        origin.source_set_complete(),
        "the fixture field has complete evidence"
    );
    assert!(
        inventory.iter_uses().any(|used| {
            origin.source_uses_refs().contains(used.id()) && used.scope() == InputScope::Payload
        }),
        "the field retains its original payload use in the query inventory"
    );
}

fn witness<Row: JsonSchema + Serialize>(
    dataset: &'static str,
    result: &ReputationRows<Row>,
    catalog: &Value,
) -> TestResult<Route> {
    let expected = catalog
        .pointer(dataset)
        .ok_or("canonical reputation table")?;
    let table = json!({"availability": result.availability, "rows": &result.rows});
    assert_eq!(
        &table, expected,
        "canonical rows and source references for {dataset}"
    );
    Route::queried_rows(dataset, &result.rows, None)
}
