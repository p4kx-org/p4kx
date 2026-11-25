// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_export::catalog_queries::equipment::EquipmentSnapshot;
use p4kx_export::catalog_queries::equipment::hacking::{
    HackableDeviceSnapshot, HackingChipSnapshot, HackingConfiguration,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, ResourceFloat32,
};
use p4kx_extraction::{DataForgeEntry, SourceValueState};
use serde::Serialize;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;
use crate::sdk_full_proof as full_proof;

#[path = "../catalog_schema/sdk_hacking_fixture.rs"]
mod fixture;

#[test]
fn hacking_sdk_preserves_canonical_rows_and_distinct_physical_owner_scopes() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(), DcbOrigin::Encrypted)?;
    let catalog: Value = serde_json::from_slice(
        build_catalog(&archive, common::options().with_full_provenance())?.json(),
    )?;
    let entry = DataForgeEntry::read(&archive)?;
    let session = full_proof::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    let chip = CatalogRecordSelection::RecordIndex(fixture::CHIP_RECORD);
    let device = CatalogRecordSelection::RecordIndex(fixture::DEVICE_RECORD);
    parsed.with_hacking_chips(chip, None, |snapshot| check_chips(&snapshot, &catalog))??;
    parsed.with_hacking_chips(device, None, |snapshot| {
        assert!(
            snapshot.rows().is_empty(),
            "chip rows require an admitted Item even if the component is present"
        );
    })?;
    session.with_hackable_devices(device, None, |snapshot| check_device(&snapshot, &catalog))??;
    parsed.with_hackable_devices(chip, None, |snapshot| {
        assert!(
            snapshot.rows().is_empty(),
            "only original physical hackable-device components qualify"
        );
    })?;
    Ok(())
}

fn check_chips(snapshot: &HackingChipSnapshot<'_>, catalog: &Value) -> TestResult {
    let [chip, removable] = snapshot.rows() else {
        return Err("one charge component and one removable component".into());
    };
    assert_eq!(
        chip.owner().record_index().value(),
        Some(&fixture::CHIP_RECORD),
        "physical Item owner stays identifiable"
    );
    let Some(HackingConfiguration::HackingChip { value }) = chip.configuration().value() else {
        return Err("typed charge settings".into());
    };
    assert_eq!(
        value.max_charges().value(),
        Some(&0),
        "zero charges are authored"
    );
    let Some(HackingConfiguration::RemovableChip { value }) = removable.configuration().value()
    else {
        return Err("typed removable modifiers".into());
    };
    let [first, second] = value.values().value().ok_or("original array")?.as_slice() else {
        return Err("both duplicate-name modifier declarations".into());
    };
    let first = first.value().ok_or("original first modifier")?;
    let second = second.value().ok_or("original second modifier")?;
    assert_eq!(
        first.value().value().map(ResourceFloat32::raw_bits),
        Some(0.0_f32.to_bits()),
        "the first physical modifier keeps positive zero"
    );
    assert_eq!(
        second.value().value().map(ResourceFloat32::raw_bits),
        Some((-0.0_f32).to_bits()),
        "the second physical modifier keeps signed-zero bits"
    );
    assert_eq!(
        second.value().origin().contains_encrypted_source(),
        Some(true),
        "the exact second modifier derives from the encrypted Entry"
    );
    parity("/configurations/hackingChips", snapshot, catalog)
}

fn check_device(snapshot: &HackableDeviceSnapshot<'_>, catalog: &Value) -> TestResult {
    let [device] = snapshot.rows() else {
        return Err("one original hackable-device component".into());
    };
    assert_eq!(
        device.owner().record_index().value(),
        Some(&fixture::DEVICE_RECORD),
        "a physical Entity outside the Item family is admitted for devices"
    );
    assert_eq!(
        device
            .base_error_chance()
            .value()
            .map(ResourceFloat32::raw_bits),
        Some((-0.0_f32).to_bits()),
        "the source error chance keeps signed-zero bits"
    );
    assert_eq!(
        device.audio().audio_trigger_success().state(),
        SourceValueState::Absent,
        "the undeclared trigger stays absent"
    );
    let start = device
        .audio()
        .audio_trigger_start()
        .value()
        .ok_or("start trigger")?;
    assert_eq!(
        start.setting().audio_trigger().value().map(String::as_str),
        Some(""),
        "the configured empty trigger is a present value"
    );
    let progress = device
        .audio()
        .audio_rtpc_overall_progress()
        .value()
        .ok_or("progress parameter")?;
    assert_eq!(
        progress
            .setting()
            .rtpc()
            .origin()
            .contains_encrypted_source(),
        Some(true),
        "the audio parameter retains per-value encryption provenance"
    );
    parity("/configurations/hackableDevices", snapshot, catalog)
}

fn parity<Row: Serialize>(
    dataset: &str,
    snapshot: &EquipmentSnapshot<'_, Row>,
    catalog: &Value,
) -> TestResult {
    assert!(
        snapshot.requested_languages().is_empty(),
        "the scoped query opens no locale inputs"
    );
    let coverage = snapshot.coverage();
    assert!(
        coverage.unreadable_headers.is_empty()
            && coverage.unreadable_bodies.is_empty()
            && coverage.incomplete_fields.is_empty(),
        "the original physical selection is read completely"
    );
    let table = catalog.pointer(dataset).ok_or("canonical dataset")?;
    assert_eq!(
        table.get("availability"),
        Some(&serde_json::to_value(snapshot.availability())?),
        "canonical availability for {dataset}"
    );
    let provenance = json!({"provenance": snapshot.source_inventory()});
    let expected = table.get("rows").ok_or("canonical rows")?;
    assert_eq!(
        proof::with_origins(serde_json::to_value(snapshot.rows())?, &provenance)?,
        proof::with_origins(expected.clone(), catalog)?,
        "rows and exact original per-datum uses for {dataset}"
    );
    Ok(())
}
