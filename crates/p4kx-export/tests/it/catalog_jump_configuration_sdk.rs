// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_export::catalog_queries::equipment::EquipmentSnapshot;
use p4kx_export::catalog_queries::equipment::component_definitions::{
    GlobalJumpConfigurationSnapshot, JumpTunnelForceSnapshot, with_jump_tunnel_forces_from,
};
use p4kx_export::catalog_queries::equipment::jump_points::JumpPointConfigurationSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ResourceFloat32,
};
use p4kx_extraction::DataForgeEntry;
use serde::Serialize;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "../catalog_schema/sdk_jump_configuration_fixture.rs"]
mod fixture;

#[test]
fn jump_configuration_queries_match_original_rows_and_origins() -> TestResult {
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
    parsed.with_jump_point_configurations(
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| check_local(&snapshot, &catalog),
    )??;
    let guid = common::guid(fixture::GLOBAL_GUID).parse()?;
    parsed.with_global_jump_point_configurations(
        CatalogRecordSelection::Guid(guid),
        None,
        |snapshot| check_globals(&snapshot, &catalog),
    )??;
    with_jump_tunnel_forces_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(3),
        None,
        |snapshot| check_tunnel(&snapshot, &catalog),
    )??;
    parsed.with_global_jump_point_configurations(
        CatalogRecordSelection::RecordIndex(3),
        None,
        |snapshot| {
            assert!(
                snapshot.rows().is_empty(),
                "the force Record is outside the global source families"
            );
            assert!(
                matches!(snapshot.availability(), DatasetAvailability::Available),
                "the scope keeps original family availability"
            );
        },
    )?;
    Ok(())
}

fn check_local(snapshot: &JumpPointConfigurationSnapshot<'_>, catalog: &Value) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one original local jump-point component".into());
    };
    assert_eq!(
        row.owner().record_index().value(),
        Some(&0),
        "the physical Entity Record"
    );
    assert_eq!(
        row.owner().component_ordinal().value(),
        Some(&1),
        "the original component slot"
    );
    assert_eq!(
        row.required_fuel().value().map(ResourceFloat32::raw_bits),
        Some(3.25_f32.to_bits()),
        "authored fuel requirement"
    );
    assert_eq!(
        row.required_fuel().origin().contains_encrypted_source(),
        Some(true),
        "source encryption for the field"
    );
    parity("/configurations/jumpPointConfigurations", snapshot, catalog)
}

fn check_globals(snapshot: &GlobalJumpConfigurationSnapshot<'_>, catalog: &Value) -> TestResult {
    let [point, host] = snapshot.rows() else {
        return Err("both original global Records with the shared GUID".into());
    };
    for (row, index, type_name) in [
        (point, 1, "GlobalJumpPointParams"),
        (host, 2, "GlobalJumpTunnelHostParams"),
    ] {
        let source = row.source();
        assert_eq!(
            source.record_index().value(),
            Some(&index),
            "physical global owner"
        );
        assert_eq!(
            source.source_type().value().copied(),
            Some(type_name),
            "original global source type"
        );
    }
    parity(
        "/configurations/globalJumpPointConfigurations",
        snapshot,
        catalog,
    )
}

fn check_tunnel(snapshot: &JumpTunnelForceSnapshot<'_>, catalog: &Value) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one original tunnel-force Record".into());
    };
    assert_eq!(
        row.source().record_index().value(),
        Some(&3),
        "physical tunnel owner"
    );
    let [root] = row.fields().roots() else {
        return Err("one original force value".into());
    };
    assert_eq!(
        root.name().value().copied(),
        Some("fixtureForce"),
        "ordered authored force member"
    );
    parity("/configurations/jumpTunnelForces", snapshot, catalog)
}

fn parity<Row: Serialize, Dependencies>(
    dataset: &str,
    snapshot: &EquipmentSnapshot<'_, Row, Dependencies>,
    catalog: &Value,
) -> TestResult {
    let coverage = snapshot.coverage();
    assert!(
        coverage.unreadable_headers.is_empty()
            && coverage.unreadable_bodies.is_empty()
            && coverage.incomplete_fields.is_empty(),
        "the selected physical source set is complete"
    );
    assert!(
        snapshot.requested_languages().is_empty(),
        "the query needs no locales"
    );
    let table = catalog.pointer(dataset).ok_or("canonical jump table")?;
    let availability = serde_json::to_value(snapshot.availability())?;
    assert_eq!(
        table.get("availability"),
        Some(&availability),
        "canonical source-family availability"
    );
    let expected = table.get("rows").ok_or("canonical jump rows")?;
    let observed = serde_json::from_slice(&serde_json::to_vec(snapshot.rows())?)?;
    let provenance = json!({"provenance": snapshot.source_inventory()});
    assert_eq!(
        proof::with_origins(observed, &provenance)?,
        proof::with_origins(expected.clone(), catalog)?,
        "{dataset} canonical field values and per-datum original sources"
    );
    Ok(())
}
