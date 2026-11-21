// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_export::catalog_queries::equipment::EquipmentSnapshot;
use p4kx_export::catalog_queries::equipment::durability::{
    DegradationSnapshot, DistortionSnapshot, DurabilitySnapshot, with_degradation_from,
    with_distortion_from,
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

#[path = "../catalog_schema/sdk_item_durability_fixture.rs"]
mod fixture;

#[test]
fn item_durability_queries_match_staged_and_resident_producer_rows() -> TestResult {
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&fixture::database(), origin)?;
        let produced = build_catalog(&archive, &common::options())?;
        let catalog: Value = serde_json::from_slice(produced.json())?;
        let entry = DataForgeEntry::read(&archive)?;
        let session = CatalogQuerySession::from_entry(
            &archive,
            &entry,
            CatalogLanguageSelection::AllAvailable,
            None,
        )?;
        let parsed = session.parsed(None)?;
        let guid = common::guid(fixture::OWNER).parse()?;
        parsed.with_durability(CatalogRecordSelection::Guid(guid), None, |snapshot| {
            check_health(&snapshot, &catalog, encrypted)
        })??;
        with_degradation_from(
            &archive,
            &entry,
            CatalogRecordSelection::RecordIndex(0),
            None,
            |snapshot| check_degradation(&snapshot, &catalog),
        )??;
        with_distortion_from(
            &archive,
            &entry,
            CatalogRecordSelection::RecordIndex(1),
            None,
            |snapshot| check_distortion(&snapshot, &catalog),
        )??;
        parsed.with_distortion(CatalogRecordSelection::RecordIndex(0), None, |snapshot| {
            assert!(
                snapshot.rows().is_empty(),
                "exact Item query excludes an adjacent owner with the same GUID"
            );
            assert!(
                matches!(snapshot.availability(), DatasetAvailability::Available),
                "empty scoped rows keep declared family availability"
            );
        })?;
    }
    Ok(())
}

fn check_health(snapshot: &DurabilitySnapshot<'_>, catalog: &Value, encrypted: bool) -> TestResult {
    let [first, repeated, second] = snapshot.rows() else {
        return Err("two Health components of the first Item and one of the second".into());
    };
    for (row, index, slot) in [(first, 0, 1), (repeated, 0, 3), (second, 1, 2)] {
        assert_eq!(
            row.owner().record_index().value(),
            Some(&index),
            "original Record order"
        );
        assert_eq!(
            row.owner().component_ordinal().value(),
            Some(&slot),
            "original slot order"
        );
        assert_eq!(
            row.health().origin().contains_encrypted_source(),
            Some(encrypted),
            "each Health datum uses the original DataForge Entry"
        );
    }
    assert_eq!(
        repeated.health().value().map(ResourceFloat32::raw_bits),
        Some(0x7fc0_1234),
        "the staged row preserves non-finite original bits"
    );
    verify(fixture::DURABILITY, snapshot, catalog)
}

fn check_degradation(snapshot: &DegradationSnapshot<'_>, catalog: &Value) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one original degradation row".into());
    };
    assert_eq!(
        row.stop_degrading_if_destroyed().value(),
        Some(&true),
        "authored degradation flag"
    );
    assert_eq!(
        row.owner().component_ordinal().value(),
        Some(&2),
        "degradation slot"
    );
    verify(fixture::DEGRADATION, snapshot, catalog)
}

fn check_distortion(snapshot: &DistortionSnapshot<'_>, catalog: &Value) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one original distortion row".into());
    };
    assert_eq!(
        row.maximum().value().map(ResourceFloat32::raw_bits),
        Some(80.0_f32.to_bits()),
        "authored distortion limit"
    );
    assert_eq!(
        row.owner().component_ordinal().value(),
        Some(&1),
        "distortion slot"
    );
    verify(fixture::DISTORTION, snapshot, catalog)
}

fn verify<Row: Serialize>(
    dataset: &str,
    snapshot: &EquipmentSnapshot<'_, Row>,
    catalog: &Value,
) -> TestResult {
    let coverage = snapshot.coverage();
    assert!(
        coverage.unreadable_headers.is_empty()
            && coverage.unreadable_bodies.is_empty()
            && coverage.incomplete_fields.is_empty(),
        "the original selected component families are complete"
    );
    assert!(
        snapshot.requested_languages().is_empty(),
        "no localization is required"
    );
    let table = catalog.pointer(dataset).ok_or("canonical table")?;
    let availability = serde_json::to_value(snapshot.availability())?;
    assert_eq!(
        table.get("availability"),
        Some(&availability),
        "canonical source-family availability for {dataset}"
    );
    let expected = table.get("rows").ok_or("canonical rows")?;
    let observed = serde_json::from_slice(&serde_json::to_vec(snapshot.rows())?)?;
    let provenance = json!({"provenance": snapshot.source_inventory()});
    assert_eq!(
        proof::with_origins(observed, &provenance)?,
        proof::with_origins(expected.clone(), catalog)?,
        "canonical {dataset} values, source selectors and encryption markers"
    );
    Ok(())
}
