// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_export::catalog_queries::equipment::EquipmentSnapshot;
use p4kx_export::catalog_queries::equipment::ordnance::{
    EmpSnapshot, ExplosionDefinitionSnapshot, ExplosiveSnapshot, MissileConfigurationSnapshot,
    with_missile_configurations_from,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ResourceFloat32,
};
use p4kx_extraction::DataForgeEntry;
use serde::Serialize;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "../catalog_schema/sdk_ordnance_fixture.rs"]
mod fixture;

#[test]
fn ordnance_sdk_preserves_item_admission_optional_references_and_producer_parity() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(), DcbOrigin::Encrypted)?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let entry = DataForgeEntry::read(&archive)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    let selection = CatalogRecordSelection::All;
    parsed.with_emp(selection, None, |snapshot| check_emp(&snapshot, &catalog))??;
    parsed.with_explosives(selection, None, |snapshot| {
        check_explosives(&snapshot, &catalog)
    })??;
    with_missile_configurations_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| check_missile(&snapshot, &catalog),
    )??;
    parsed.with_explosion_definitions(selection, None, |snapshot| {
        check_definitions(&snapshot, &catalog)
    })??;
    parsed.with_emp(CatalogRecordSelection::RecordIndex(1), None, |snapshot| {
        assert!(
            snapshot.rows().is_empty(),
            "non-Item physical owner is outside the admitted domain"
        );
        assert!(
            matches!(snapshot.availability(), DatasetAvailability::Available),
            "the declared EMP family remains available"
        );
    })?;
    Ok(())
}

fn check_emp(snapshot: &EmpSnapshot<'_>, catalog: &Value) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one canonical EMP row".into());
    };
    assert_eq!(
        row.owner().record_index().value(),
        Some(&0),
        "the admitted physical Item"
    );
    assert_eq!(
        row.distortion_damage()
            .value()
            .map(ResourceFloat32::raw_bits),
        Some(120.0_f32.to_bits()),
        "the exact authored value"
    );
    assert_eq!(
        row.distortion_damage().origin().contains_encrypted_source(),
        Some(true),
        "the field is bound to the encrypted DataForge Entry"
    );
    let actual = wire(row)?;
    proof::cell(&actual, "chargedTag", "null")?;
    let dangling = proof::at(&actual, "/chargingTag/value")?;
    assert_eq!(
        proof::at(dangling, "/status")?,
        "dangling",
        "the absent target stays unresolved"
    );
    proof::present(
        dangling,
        "guid",
        &json!(common::guid(fixture::MISSING_TAG)),
        Some(true),
    )?;
    parity("/configurations/emp", snapshot, catalog)
}

fn check_explosives(snapshot: &ExplosiveSnapshot<'_>, catalog: &Value) -> TestResult {
    let [missile, ordnance] = snapshot.rows() else {
        return Err("one original missile and one ordinary explosive".into());
    };
    assert_eq!(
        missile.owner().component_ordinal().value(),
        Some(&2),
        "original ordnance slot"
    );
    assert_eq!(
        ordnance.owner().component_ordinal().value(),
        Some(&3),
        "ordinary explosive slot"
    );
    parity("/configurations/explosives", snapshot, catalog)
}

fn check_missile(snapshot: &MissileConfigurationSnapshot<'_>, catalog: &Value) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("only the original missile owner".into());
    };
    assert_eq!(
        row.owner().component_ordinal().value(),
        Some(&2),
        "same physical slot as the ordnance row"
    );
    let observed = wire(row)?;
    proof::cell(&observed, "guidance", "absent")?;
    proof::cell(&observed, "shake", "null")?;
    parity("/configurations/missileConfigurations", snapshot, catalog)
}

fn check_definitions(snapshot: &ExplosionDefinitionSnapshot<'_>, catalog: &Value) -> TestResult {
    let [first, second] = snapshot.rows() else {
        return Err("both original independent explosion Records".into());
    };
    assert_eq!(
        first.source().record_index().value(),
        Some(&3),
        "first original GUID in canonical order"
    );
    assert_eq!(
        second.source().record_index().value(),
        Some(&2),
        "second original GUID in canonical order"
    );
    parity("/definitions/explosionDefinitions", snapshot, catalog)
}

fn wire(value: &impl Serialize) -> TestResult<Value> {
    Ok(serde_json::from_slice(&serde_json::to_vec(value)?)?)
}

fn parity<Row: Serialize>(
    dataset: &str,
    snapshot: &EquipmentSnapshot<'_, Row>,
    catalog: &Value,
) -> TestResult {
    let coverage = snapshot.coverage();
    assert!(
        coverage.unreadable_headers.is_empty()
            && coverage.unreadable_bodies.is_empty()
            && coverage.incomplete_fields.is_empty(),
        "authored absent and dangling references preserve complete known identity"
    );
    assert!(
        snapshot.requested_languages().is_empty(),
        "the scope opens no locale inputs"
    );
    let table = catalog.pointer(dataset).ok_or("canonical dataset")?;
    let availability = serde_json::to_value(snapshot.availability())?;
    assert_eq!(
        table.get("availability"),
        Some(&availability),
        "canonical availability for {dataset}"
    );
    let expected = table.get("rows").ok_or("canonical rows")?;
    let provenance = json!({"provenance": snapshot.source_inventory()});
    assert_eq!(
        proof::with_origins(wire(&snapshot.rows())?, &provenance)?,
        proof::with_origins(expected.clone(), catalog)?,
        "{dataset} canonical rows and their original per-datum uses"
    );
    Ok(())
}
