// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::component_definitions::JumpDriveDefinition;
use p4kx_export::catalog_queries::equipment::ship_systems::JumpDriveSnapshot;
use p4kx_export::catalog_queries::{CatalogRecordSelection, ResourceNumber};
use p4kx_extraction::{Archive, DataForgeEntry, SourceInventory};
use serde_json::{Value, json};

use super::{DcbOrigin, ROWS, TestResult, build_catalog, common, database, proof};
use crate::sdk_full_proof as full_proof;

const TUNING_ROWS: &str = "/configurations/jumpDriveFlightTuning/rows";

#[test]
fn jump_drive_sdk_keeps_device_and_flight_tuning_original_sources() -> TestResult {
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database(false), origin)?;
        let catalog: Value = serde_json::from_slice(
            build_catalog(&archive, common::options().with_full_provenance())?.json(),
        )?;
        let [device] = proof::rows(&catalog, ROWS)? else {
            return Err("expected one original jump drive".into());
        };
        let [first, second] = proof::rows(&catalog, TUNING_ROWS)? else {
            return Err("expected both original flight-tuning Records".into());
        };
        full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?.with_jump_drives(
            CatalogRecordSelection::RecordIndex(0),
            None,
            |snapshot| check_device(&snapshot, &catalog, device, encrypted),
        )??;
        full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?
            .with_jump_drive_flight_tuning(CatalogRecordSelection::All, None, |snapshot| {
                let [first_row, second_row] = snapshot.rows() else {
                    return Err("expected both original tuning rows in source order".into());
                };
                assert!(
                    snapshot.coverage().incomplete_fields.is_empty(),
                    "readable tuning fields have complete coverage"
                );
                let inventory = snapshot.source_inventory();
                check_definition(first_row, first, inventory, &catalog, 1, encrypted)?;
                check_definition(second_row, second, inventory, &catalog, 2, encrypted)
            })??;
        check_preloaded_tuning(&archive, &catalog, second, encrypted)?;
    }
    Ok(())
}

fn check_preloaded_tuning(
    archive: &Archive,
    catalog: &Value,
    second: &Value,
    encrypted: bool,
) -> TestResult {
    let entry = DataForgeEntry::read(archive)?;
    full_proof::from_entry(archive, &entry, full_proof::NO_LANGUAGES, None)?
        .with_jump_drive_flight_tuning(
            CatalogRecordSelection::RecordIndex(2),
            None,
            |snapshot| {
                let [row] = snapshot.rows() else {
                    return Err("one exact original tuning Record".into());
                };
                check_definition(
                    row,
                    second,
                    snapshot.source_inventory(),
                    catalog,
                    2,
                    encrypted,
                )
            },
        )??;
    full_proof::from_entry(archive, &entry, full_proof::NO_LANGUAGES, None)?
        .with_jump_drive_flight_tuning(
            CatalogRecordSelection::RecordIndex(0),
            None,
            |snapshot| {
                assert!(
                    snapshot.rows().is_empty(),
                    "a device Record is not a flight-tuning owner"
                );
            },
        )?;
    Ok(())
}

fn check_device(
    snapshot: &JumpDriveSnapshot<'_>,
    catalog: &Value,
    expected: &Value,
    encrypted: bool,
) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one physical jump drive component is expected".into());
    };
    assert_eq!(
        row.owner().component_ordinal().value(),
        Some(&0),
        "the physical component occurrence is preserved"
    );
    assert!(
        snapshot.coverage().incomplete_fields.is_empty(),
        "all required device fields were read"
    );
    let alignment = row.alignment_rate();
    assert!(
        matches!(alignment.value(), Some(ResourceNumber::Single(value)) if value.raw_bits() == 0),
        "the authored alignment rate is a stored Single"
    );
    assert_eq!(
        alignment.origin().contains_encrypted_source(),
        Some(encrypted),
        "the device value uses the selected Archive's DataForge"
    );
    let sdk = json!({"provenance":snapshot.source_inventory()});
    let row = serde_json::from_slice(&serde_json::to_vec(row)?)?;
    assert_eq!(
        proof::with_origins(row, &sdk)?,
        proof::with_origins(expected.clone(), catalog)?,
        "the scoped device and catalog retain the same original values and evidence"
    );
    Ok(())
}

fn check_definition(
    row: &JumpDriveDefinition<'_>,
    expected: &Value,
    inventory: &SourceInventory,
    catalog: &Value,
    index: u32,
    encrypted: bool,
) -> TestResult {
    assert_eq!(
        row.source().record_index().value(),
        Some(&index),
        "the tuning query preserves the physical Record identity"
    );
    let [field] = row.fields().roots() else {
        return Err("expected the original tuning field".into());
    };
    assert_eq!(
        field.name().value(),
        Some(&"respoolTime"),
        "the tuning field remains directly addressable"
    );
    assert_eq!(
        field.name().origin().contains_encrypted_source(),
        Some(encrypted),
        "the field receipt identifies the selected DataForge source"
    );
    let sdk = json!({"provenance":inventory});
    let row = serde_json::from_slice(&serde_json::to_vec(row)?)?;
    assert_eq!(
        proof::with_origins(row, &sdk)?,
        proof::with_origins(expected.clone(), catalog)?,
        "the scoped definition and catalog retain the same original values and evidence"
    );
    Ok(())
}
