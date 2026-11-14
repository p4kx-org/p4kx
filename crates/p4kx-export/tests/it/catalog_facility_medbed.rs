// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_extraction::{Archive, DataForgeEntry, Value as Original};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_facility_medbed_fixture.rs"]
mod fixture;

#[test]
fn medbed_configuration_and_tier_sources_join_at_their_original_occurrences() -> TestResult {
    let (_directory, archive) = common::open(&fixture::database(), DcbOrigin::Encrypted)?;
    verify_original(&archive)?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let [component] = proof::rows(&catalog, "/configurations/facilities/rows")? else {
        return Err("one physical MedBed facility component".into());
    };
    check_component(component)?;
    check_surgery_text(&catalog, component)?;
    check_placement(&catalog)?;
    let [configuration] = proof::rows(&catalog, "/definitions/medicalItemTierConfigs/rows")? else {
        return Err("one physical MedicalItemTierConfig definition".into());
    };
    check_configuration(configuration)?;
    jsonschema::validator_for(&p4kx_export::catalog_schema())?
        .validate(&catalog)
        .map_err(|error| format!("schema: {error}"))?;
    proof::closure(&catalog)
}

fn verify_original(archive: &Archive) -> TestResult {
    let dcb = DataForgeEntry::read(archive)?;
    let forge = dcb.open()?;
    let tree = forge.record(0).ok_or("original fixture owner")?.tree()?;
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("original Components array".into());
    };
    let Some(Original::Struct(component)) = components.get(1) else {
        return Err("original physical MedBed slot".into());
    };
    assert_eq!(
        component.type_name(),
        fixture::COMPONENT,
        "original component type"
    );
    assert!(
        matches!(component.field("healInteraction"), Some(Original::Reference(target)) if target.location().is_some()),
        "actual original WeakPointer resolves to an owned interaction"
    );
    let target = component.field("medicalItemTierConfig");
    assert!(
        matches!(target, Some(Original::Reference(target)) if target.record_index() == 1),
        "the actual authoring selector resolves to the original tier Record"
    );
    Ok(())
}

fn check_component(component: &Value) -> TestResult {
    assert_eq!(
        component["kind"], "medBed",
        "one authored MedBed facility type"
    );
    for (key, value) in [
        ("ownerGuid", json!(common::guid(fixture::OWNER))),
        ("ownerRecordIndex", json!(0)),
        ("componentOrdinal", json!(1)),
        ("sourceType", json!(fixture::COMPONENT)),
        ("medBedTier", json!("FixtureTierTwo")),
        ("canRespawnHere", json!(false)),
    ] {
        proof::present(component, key, &value, Some(true))?;
    }
    let selector = proof::at(component, "/medicalItemTierConfig/value")?;
    proof::present(
        selector,
        "guid",
        &json!(common::guid(fixture::TIER_CONFIG)),
        Some(true),
    )?;
    proof::present(selector, "targetRecordIndex", &json!(1), Some(true))?;
    let heal = proof::at(component, "/healInteraction/value")?;
    proof::present(
        heal,
        "recordGuid",
        &json!(common::guid(fixture::OWNER)),
        Some(true),
    )?;
    proof::cell(component, "setRespawnInteraction", "null")?;
    let range = proof::at(component, "/respawnRangeOverride/value")?;
    proof::present(range, "respawnRange", &json!(0), Some(true))?;
    let capacity = proof::at(component, "/resourceRegenerationPerMinute/value")?;
    proof::present(capacity, "microSCU", &json!(12), Some(true))?;
    proof::present(
        component,
        "timeToHeal",
        &json!({"rawBits":"80000000","finite":-0.0}),
        Some(true),
    )?;
    Ok(())
}

fn check_surgery_text(catalog: &Value, component: &Value) -> TestResult {
    let names = proof::at(component, "/surgeryNames/value/headInjuries/value")?;
    let label = proof::cell(names, "injuryName", "present")?;
    let id = proof::at(label, "/value")?;
    let row = common::text_row(catalog, "gameTexts", id)?;
    assert_eq!(
        proof::at(row, "/translations/value/en/sourceText/value")?,
        "Shared text",
        "the authored injury label uses the protected game-text definition"
    );
    Ok(())
}

fn check_placement(catalog: &Value) -> TestResult {
    let [instance] = proof::rows(catalog, "/placements/facilityInstances/rows")? else {
        return Err("one physical MedBed Loadout placement".into());
    };
    proof::present(
        instance,
        "shipGuid",
        &json!(common::guid(fixture::SHIP)),
        Some(true),
    )?;
    assert_eq!(
        proof::at(instance, "/source/position/value/portOrdinals")?,
        &json!([0]),
        "the facility belongs to the original host port"
    );
    let owner = proof::at(instance, "/configurations/0/owner")?;
    proof::present(
        owner,
        "ownerGuid",
        &json!(common::guid(fixture::OWNER)),
        Some(true),
    )?;
    proof::present(owner, "ownerRecordIndex", &json!(0), Some(true))?;
    proof::present(owner, "componentOrdinal", &json!(1), Some(true))?;
    Ok(())
}

fn check_configuration(configuration: &Value) -> TestResult {
    proof::present(
        configuration,
        "guid",
        &json!(common::guid(fixture::TIER_CONFIG)),
        Some(true),
    )?;
    proof::present(configuration, "recordIndex", &json!(1), Some(true))?;
    let tiers = proof::rows(configuration, "/medBedConfigs/value")?;
    assert_eq!(tiers.len(), 3, "duplicate original tier slots survive");
    for (tier, (label, range)) in tiers.iter().zip([
        ("FixtureTierTwo", 0),
        ("FixtureTierOne", 73_001),
        ("FixtureTierTwo", 0),
    ]) {
        let tier = proof::at(tier, "/value")?;
        proof::present(tier, "medBedTier", &json!(label), Some(true))?;
        proof::present(tier, "defaultRespawnRange", &json!(range), Some(true))?;
        let stats = proof::rows(tier, "/statTypesToHeal/value")?;
        assert_eq!(
            stats
                .iter()
                .filter_map(|stat| stat.get("value").and_then(Value::as_str))
                .collect::<Vec<_>>(),
            ["FixtureStatTwo", "FixtureStatOne", "FixtureStatTwo"],
            "all authored stat slots retain their order"
        );
    }
    let consumption = proof::at(configuration, "/resourceConsumption/value")?;
    let resource = proof::at(consumption, "/bedResource/value")?;
    proof::present(
        resource,
        "guid",
        &json!(common::guid(fixture::RESOURCE)),
        Some(true),
    )?;
    let first = proof::at(consumption, "/resourceAmountForSurgeryT1/value")?;
    proof::present(
        first,
        "standardCargoUnits",
        &json!({"rawBits":"3f400000","finite":0.75}),
        Some(true),
    )?;
    let second = proof::at(consumption, "/resourceAmountForSurgeryT2/value")?;
    proof::present(second, "centiSCU", &json!(9), Some(true))?;
    let third = proof::at(consumption, "/resourceAmountForSurgeryT3/value")?;
    assert_eq!(
        third["kind"], "unknownUnit",
        "unknown original cargo unit survives"
    );
    proof::cell(consumption, "resourceAmountForCloning", "null")?;
    Ok(())
}
