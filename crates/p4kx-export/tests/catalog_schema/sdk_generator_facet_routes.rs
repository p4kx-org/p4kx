// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{DataForgeEntry, FieldOrigin, InputScope, SourceInventory, SourceValueState};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};

use super::super::{CONTRACT_SCENARIO_REQUIREMENTS, LOCAL_CONTRACT_GENERATORS};
use super::{Route, TestResult};

#[path = "sdk_generator_facet_fixture.rs"]
mod fixture;

pub(super) fn read() -> TestResult<[Route; 2]> {
    let (_folder, archive) = fixture::open(fixture::Input::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(&archive, &entry, options.with_full_provenance())?.json(),
    )?;
    full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
        .with_contract_generator_facets(CatalogRecordSelection::All, None, |snapshot| {
            for row in &snapshot.local_generators {
                let scope = row.scope().ok_or("authored local scope")?;
                assert_origin(scope.origin(), &snapshot.source_inventory);
            }
            for row in &snapshot.scenario_requirements {
                let slots = row.scenarios().value().ok_or("authored scenario list")?;
                let [target, null] = slots.as_slice() else {
                    return Err("both original scenario slots".into());
                };
                assert_origin(target.origin(), &snapshot.source_inventory);
                assert_eq!(
                    null.state(),
                    SourceValueState::Null,
                    "the authored null keeps its position"
                );
            }
            Ok([
                witness(
                    LOCAL_CONTRACT_GENERATORS,
                    snapshot.local_generator_availability,
                    &snapshot.local_generators,
                    &catalog,
                )?,
                witness(
                    CONTRACT_SCENARIO_REQUIREMENTS,
                    snapshot.scenario_requirement_availability,
                    &snapshot.scenario_requirements,
                    &catalog,
                )?,
            ])
        })?
}

fn assert_origin(origin: &FieldOrigin, inventory: &SourceInventory) {
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "the exact fixture datum came from the encrypted DataForge Entry"
    );
    assert!(
        origin.source_set_complete()
            && inventory.iter_uses().any(|used| {
                used.scope() == InputScope::Payload && origin.source_uses_refs().contains(used.id())
            }),
        "the source is closed over the original payload receipt"
    );
}

fn witness<Row: JsonSchema + Serialize>(
    dataset: &'static str,
    availability: DatasetAvailability,
    rows: &[Row],
    catalog: &Value,
) -> TestResult<Route> {
    let expected = catalog
        .pointer(dataset)
        .ok_or("canonical Generator facet table")?;
    let table = json!({"availability": availability, "rows": rows});
    assert_eq!(
        &table, expected,
        "the canonical table and source references at {dataset}"
    );
    Route::queried_rows(
        dataset,
        rows,
        "encrypted synthetic producer parity available; installed producer parity open",
    )
}

#[test]
fn generator_facets_stay_within_selected_owner_and_reuse_its_arena_references() -> TestResult<()> {
    let (_folder, archive) = fixture::open(fixture::Input::LockedLocale)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    let selection = CatalogRecordSelection::RecordIndex(fixture::PRIMARY_INDEX);
    parsed.with_contract_generator_facets(selection, None, |snapshot| -> TestResult<()> {
        let ([local], [scenario]) = (
            snapshot.local_generators.as_slice(),
            snapshot.scenario_requirements.as_slice(),
        ) else {
            return Err("only the selected Generator supplies both authored facets".into());
        };
        assert_eq!(
            local.record_index(),
            fixture::PRIMARY_INDEX,
            "selected physical Handler owner"
        );
        assert_eq!(
            scenario.record_index(),
            fixture::PRIMARY_INDEX,
            "selected physical scenario-list owner"
        );
        assert_eq!(
            local.scope().and_then(|scope| scope.value()),
            Some(&"FixturePlanet"),
            "the unselected Generator does not provide source data"
        );
        parsed.with_contract_generators(selection, None, |definitions| -> TestResult<()> {
            let [owner] = definitions.rows.as_slice() else {
                return Err("one independent raw Generator arena".into());
            };
            assert_eq!(
                owner.input_ref(),
                local.input_ref(),
                "the Handler has the same physical input"
            );
            assert_eq!(
                owner.input_ref(),
                scenario.input_ref(),
                "the scenario list has the same physical input"
            );
            let arena = owner.source();
            assert!(
                arena.value(local.handler()).is_some()
                    && arena.value(scenario.scenarios_value()).is_some(),
                "the selective facet references resolve in the original Generator arena"
            );
            Ok(())
        })?
    })?
}
