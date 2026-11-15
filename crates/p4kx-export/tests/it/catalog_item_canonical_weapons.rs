// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::ammo_source::AmmoSource;
use super::{catalog, catalog_from, item, owner, single};
use crate::commodity_v3_fixture::TestResult;
use crate::health_origin_assertions as proof;
use crate::item_stats_fixture::{CYCLIC_AMMO_GUN, DANGLING_GUN, GUN, PERSONAL_GUN, WRONG_TYPE_GUN};

const ACTIONS: &str = "/configurations/weaponActions/rows";
const CONTAINERS: &str = "/configurations/ammoContainers/rows";
const BINDINGS: &str = "/declarations/ammoBindings/rows";
const MAGAZINES: &str = "/projections/weaponMagazineResolutions/rows";
const AMMO: &str = "/definitions/ammoDefinitions/rows";

#[test]
fn fire_modes_keep_their_original_shot_fields_and_charge_edge() -> TestResult {
    let catalog = catalog()?;
    let weapon = owner(&catalog, ACTIONS, GUN)?;
    let [first, second] = proof::rows(weapon, "/fireActions/value")? else {
        return Err("both actual fire slots".into());
    };
    assert_eq!(proof::at(first, "/value")?, 0);
    assert_eq!(proof::at(second, "/value")?, 1);
    let [rapid, charged, child] = proof::rows(weapon, "/actionNodes")? else {
        return Err("top-level original actions and the charged child".into());
    };
    assert_rapid(rapid, 600)?;
    assert_eq!(proof::at(charged, "/kind/type")?, "charged");
    let charge = proof::at(charged, "/kind/value")?;
    for (field, value) in [
        ("chargeTime", 1.5),
        ("overchargeTime", 3.0),
        ("cooldownTime", 0.25),
    ] {
        single(charge, field, value)?;
    }
    assert_eq!(proof::at(charge, "/weaponAction/value")?, 2);
    assert_rapid(child, 120)?;
    proof::closure(&catalog)
}

fn assert_rapid(node: &Value, rpm: i32) -> TestResult {
    assert_eq!(
        proof::at(node, "/kind/type")?,
        "rapid",
        "an authored rapid action"
    );
    proof::present(
        node,
        "sourceType",
        &json!("SWeaponActionFireRapidParams"),
        Some(true),
    )?;
    let shot = proof::at(node, "/kind/value")?;
    proof::present(shot, "fireRate", &json!(rpm), Some(true))?;
    single(shot, "heatPerShot", 0.5)?;
    let raw_name = proof::at(shot, "/authored/roots/0")?;
    proof::present(raw_name, "name", &json!("name"), Some(true))?;
    let value_id = proof::at(raw_name, "/value/valueIndex")?
        .as_str()
        .ok_or("raw name id")?;
    let name = proof::at(shot, &format!("/authored/values/{value_id}"))?;
    proof::present(
        name,
        "value",
        &json!({"type":"string", "value":{"type":"gameText", "sourceText":"rapid"}}),
        Some(true),
    )?;
    let launcher = proof::at(shot, "/launchParams/value")?;
    assert_eq!(
        proof::at(launcher, "/type")?,
        "projectile",
        "the originally owned launcher"
    );
    proof::present(launcher, "pelletCount", &json!(1), Some(true))?;
    proof::present(launcher, "ammoCost", &json!(1), Some(true))?;
    let spread = proof::at(launcher, "/spreadParams/value")?;
    for (field, value) in [
        ("min", 0.18),
        ("max", 5.0),
        ("firstAttack", 1.35),
        ("attack", 1.25),
        ("decay", 6.5),
    ] {
        single(spread, field, value)?;
    }
    Ok(())
}

#[test]
fn own_and_linked_capacity_resolve_into_one_original_ammo_definition() -> TestResult {
    let catalog = catalog()?;
    let own = primary_ammo(&catalog, GUN, "ownContainer", 200, true)?;
    let linked = primary_ammo(&catalog, PERSONAL_GUN, "linkedContainer", 30, true)?;
    let ammo = ammo_definition(&catalog, own)?;
    assert_eq!(
        proof::at(own, "/targetRecordIndex/value")?,
        proof::at(linked, "/targetRecordIndex/value")?
    );
    single(ammo, "speed", 1100.0)?;
    single(ammo, "lifetime", 2.5)?;
    let projectile = proof::at(ammo, "/projectileParams/value")?;
    assert_eq!(proof::at(projectile, "/kind/type")?, "bullet");
    channels(
        proof::at(projectile, "/kind/value/damage/value")?,
        30.0,
        2.0,
    )?;
    let explosion = proof::at(projectile, "/detonationParams/value/explosionParams/value")?;
    single(explosion, "minRadius", 1.0)?;
    single(explosion, "maxRadius", 4.0)?;
    channels(proof::at(explosion, "/damage/value")?, 7.0, 0.0)?;
    proof::closure(&catalog)
}

#[test]
fn unavailable_ammo_keeps_the_container_and_reason() -> TestResult {
    let catalog = catalog()?;
    for (guid, status) in [
        (DANGLING_GUN, "dangling"),
        (WRONG_TYPE_GUN, "unexpectedTargetType"),
    ] {
        item(&catalog, guid)?;
        let reference = primary_ammo(&catalog, guid, "ownContainer", 200, true)?;
        assert_eq!(proof::at(reference, "/status")?, status);
        assert!(ammo_definition(&catalog, reference).is_err());
    }
    proof::closure(&catalog)
}

#[test]
fn cyclic_ammo_keeps_readable_scalars_and_the_original_null_projectile() -> TestResult {
    let catalog = catalog()?;
    let reference = primary_ammo(&catalog, CYCLIC_AMMO_GUN, "ownContainer", 200, true)?;
    let ammo = ammo_definition(&catalog, reference)?;
    single(ammo, "speed", 0.0)?;
    single(ammo, "lifetime", 0.0)?;
    proof::cell(ammo, "projectileParams", "null")?;
    proof::closure(&catalog)
}

#[test]
fn unbuildable_ammo_retains_its_reference_and_readable_source_capacity() -> TestResult {
    let (_directory, archive) = super::ammo_source::archive(AmmoSource::InvalidLayout)?;
    let catalog = catalog_from(&archive)?;
    item(&catalog, CYCLIC_AMMO_GUN)?;
    let reference = primary_ammo(&catalog, CYCLIC_AMMO_GUN, "ownContainer", 200, true)?;
    assert_eq!(proof::at(reference, "/targetRecordIndex/value")?, 1);
    assert!(ammo_definition(&catalog, reference).is_err());
    proof::closure(&catalog)
}

pub(super) fn primary_ammo<'catalog>(
    catalog: &'catalog Value,
    weapon: u128,
    route: &str,
    capacity: i32,
    encrypted: bool,
) -> TestResult<&'catalog Value> {
    let resolution = owner(catalog, MAGAZINES, weapon)?;
    let outcome = proof::at(resolution, "/outcome/value")?;
    assert_eq!(
        proof::at(outcome, "/type")?,
        route,
        "the actual primary route"
    );
    let record = if route == "ownContainer" {
        proof::at(resolution, "/ownerRecordIndex/value")?
    } else {
        proof::at(outcome, "/magazineRecordIndex/value")?
    };
    let slot = proof::at(outcome, "/containerOrdinal/value")?;
    let container = proof::rows(catalog, CONTAINERS)?
        .iter()
        .find(|row| same_slot(row, record, slot))
        .ok_or("selected magazine container")?;
    proof::present(container, "maxAmmoCount", &json!(capacity), Some(encrypted))?;
    let binding = proof::rows(catalog, BINDINGS)?
        .iter()
        .filter(|row| row.get("type").and_then(Value::as_str) == Some("containerSlots"))
        .filter_map(|row| row.get("value"))
        .find(|row| same_slot(row, record, slot))
        .ok_or("same physical container binding")?;
    proof::at(binding, "/primaryAmmo/value")
}

fn same_slot(row: &Value, record: &Value, slot: &Value) -> bool {
    row.pointer("/ownerRecordIndex/value") == Some(record)
        && row.pointer("/componentOrdinal/value") == Some(slot)
}

pub(super) fn ammo_definition<'catalog>(
    catalog: &'catalog Value,
    link: &Value,
) -> TestResult<&'catalog Value> {
    let target = proof::at(link, "/targetRecordIndex/value")?;
    proof::rows(catalog, AMMO)?
        .iter()
        .find(|row| row.pointer("/recordIndex/value") == Some(target))
        .ok_or_else(|| "the reference has no emitted Ammo definition".into())
}

pub(super) fn channels(damage: &Value, physical: f32, energy: f32) -> TestResult {
    assert_eq!(
        proof::at(damage, "/type")?,
        "info",
        "the original damage channel declaration"
    );
    let channels = proof::at(damage, "/value")?;
    single(channels, "physical", physical)?;
    single(channels, "energy", energy)?;
    for field in ["distortion", "thermal", "biochemical", "stun"] {
        proof::cell(channels, field, "absent")?;
    }
    Ok(())
}
