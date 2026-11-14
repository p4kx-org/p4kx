// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_weapon_operating_fixture.rs"]
mod fixture;

#[test]
fn weapon_operating_configuration_keeps_authored_states_and_provenance() -> TestResult {
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&fixture::database(), origin)?;
        let filter = p4kx_extraction::Filter::new("*/entities/scitem/*");
        let options = p4kx_export::CatalogOptions {
            filter: Some(&filter),
            ..common::options()
        };
        let built = build_catalog(&archive, &options)?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        let row = proof::at(&catalog, "/configurations/weaponActions/rows/0")?;
        let configuration = proof::at(row, "/operatingConfiguration")?;
        check_links(configuration, encrypted)?;
        check_mannequin(&catalog, row, encrypted)?;
        check_connection(configuration, encrypted)?;
        let degradation = proof::at(
            configuration,
            "/weaponDegradationModifier/value/weaponStats/value",
        )?;
        check_single(degradation, "damageMultiplier", "3f400000", encrypted)?;
        check_aim(row, encrypted)?;
        check_action_recoil(&catalog, row, encrypted)?;
        let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
        validator
            .validate(&catalog)
            .map_err(|error| error.to_string())?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn check_mannequin(catalog: &Value, weapon: &Value, encrypted: bool) -> TestResult {
    let [row] = proof::rows(catalog, "/configurations/entityAnimationControllers/rows")? else {
        return Err("one physical animation controller component".into());
    };
    assert_eq!(
        row.get("ownerRecordIndex"),
        weapon.get("ownerRecordIndex"),
        "weapon and controller keep their common Entity identity"
    );
    proof::present(row, "componentOrdinal", &json!(2), Some(encrypted))?;
    proof::present(
        row,
        "animationController",
        &json!("Animations/fixture-controller.xml"),
        Some(encrypted),
    )?;
    proof::present(
        row,
        "animationDatabase",
        &json!("Animations/fixture.adb"),
        Some(encrypted),
    )?;
    Ok(())
}

fn check_links(configuration: &Value, encrypted: bool) -> TestResult {
    let animation = proof::cell(configuration, "proceduralAnimationRecord", "present")?;
    assert_eq!(
        proof::at(animation, "/value/status")?,
        "present",
        "animation link resolves"
    );
    let animation = proof::at(animation, "/value")?;
    proof::present(animation, "targetRecordIndex", &json!(0), Some(encrypted))?;
    let recoil = proof::cell(configuration, "actorProceduralRecoilConfig", "present")?;
    let recoil = proof::at(recoil, "/value")?;
    assert_eq!(
        proof::at(recoil, "/status")?,
        "dangling",
        "recoil target is missing"
    );
    proof::present(
        recoil,
        "guid",
        &json!(common::guid(0x9af2)),
        Some(encrypted),
    )?;
    Ok(())
}

fn check_connection(configuration: &Value, encrypted: bool) -> TestResult {
    let connection = proof::at(configuration, "/connectionParams/value")?;
    proof::present(
        connection,
        "sourceType",
        &json!("SWeaponConnectionParams"),
        Some(encrypted),
    )?;
    proof::cell(connection, "noPowerStats", "absent")?;
    proof::cell(connection, "overheatLockedStats", "null")?;
    for (field, bits) in [
        ("underpowerStats", "80000000"),
        ("overclockStats", "40000000"),
    ] {
        let stats = proof::at(proof::cell(connection, field, "present")?, "/value")?;
        check_single(stats, "damageMultiplier", bits, encrypted)?;
    }
    check_single(connection, "heatRateOnline", "3e800000", encrypted)
}

fn check_aim(row: &Value, encrypted: bool) -> TestResult {
    proof::present(row, "aimAction", &json!(2), Some(encrypted))?;
    let aim = proof::at(row, "/actionNodes/2/kind")?;
    assert_eq!(
        proof::at(aim, "/type")?,
        "aim",
        "the original slot resolves to the typed aim"
    );
    let aim = proof::at(aim, "/value")?;
    proof::present(aim, "hasReloadModesOnUI", &json!(false), Some(encrypted))?;
    let modifier = proof::at(aim, "/aimModifier/value/weaponStats/value")?;
    check_single(modifier, "damageMultiplier", "3f000000", encrypted)?;
    let geometry = proof::at(aim, "/aimGeometryTags/value/additionalFields")?;
    let flag = proof::at(geometry, "/values/0")?;
    proof::present(
        flag,
        "value",
        &json!({"type": "boolean", "value": false}),
        Some(encrypted),
    )?;
    Ok(())
}

fn check_action_recoil(catalog: &Value, row: &Value, encrypted: bool) -> TestResult {
    let [first, second, aim] = proof::rows(row, "/actionNodes")? else {
        return Err("two beams and the independent aim slot".into());
    };
    let [definition] = proof::rows(catalog, "/definitions/weaponActionRecoilDefinitions/rows")?
    else {
        return Err("one physical shared recoil definition across both action nodes".into());
    };
    proof::present(
        proof::at(definition, "/source")?,
        "recordIndex",
        &json!(1),
        Some(encrypted),
    )?;
    for (node, bits, finite) in [(first, "3e000000", 0.125), (second, "3e800000", 0.25)] {
        let link = proof::at(proof::cell(node, "recoil", "present")?, "/value")?;
        proof::present(link, "targetRecordIndex", &json!(1), Some(encrypted))?;
        proof::present(
            node,
            "recoilInterval",
            &json!({"rawBits": bits, "finite": finite}),
            Some(encrypted),
        )?;
        let [original] = proof::rows(node, "/kind/value/authored/roots")? else {
            return Err("inherited UI flag remains outside the projected recoil fields".into());
        };
        proof::present(
            original,
            "name",
            &json!("hasReloadModesOnUI"),
            Some(encrypted),
        )?;
    }
    proof::cell(aim, "recoil", "absent")?;
    check_single(
        proof::at(definition, "/aim/value")?,
        "yawMaxDegrees",
        "3f800000",
        encrypted,
    )
}

fn check_single(stats: &Value, name: &str, bits: &str, encrypted: bool) -> TestResult {
    let arena = proof::at(stats, "/additionalFields")?;
    let [root] = proof::rows(arena, "/roots")? else {
        return Err("one original field outside the typed cells".into());
    };
    proof::present(root, "name", &json!(name), Some(encrypted))?;
    let value = proof::cell(proof::at(arena, "/values/0")?, "value", "present")?;
    assert_eq!(
        proof::at(value, "/value/value/rawBits")?,
        bits,
        "the exact source Single"
    );
    assert_eq!(
        proof::at(value, "/containsEncryptedSource")?,
        encrypted,
        "the source value's origin"
    );
    Ok(())
}
