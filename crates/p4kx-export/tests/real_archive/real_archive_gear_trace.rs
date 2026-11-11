// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Every gameplay property a blueprint modifies finds its stat field on the item the blueprint
//! produces, on a real Archive; runs with `--ignored` when `P4KX_DATA_P4K` names one.

use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, Kind, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::real_archive_path;
use serde_json::Value;

use crate::catalog_v3_consumer;
use crate::real_archive_blueprints::Blueprints;

type TestResult = Result<(), Box<dyn Error>>;

const FIELDS: [(&str, &[&str]); 29] = [
    (
        "gpp_armor_damagemitigation",
        &["armor.damageResistance.physical.multiplier"],
    ),
    (
        "gpp_armor_temperaturemin",
        &["clothing.temperatureResistanceMin"],
    ),
    (
        "gpp_armor_temperaturemax",
        &["clothing.temperatureResistanceMax"],
    ),
    (
        "gpp_armor_radiationcapacity",
        &["clothing.radiationCapacity"],
    ),
    (
        "gpp_armor_radiationdissipation",
        &["clothing.radiationDissipationRate"],
    ),
    ("gpp_health_maxhealth", &["health"]),
    (
        "gpp_weapon_damage",
        &["weapon.ammo.damage", "weapon.fireActions[].damagePerSecond"],
    ),
    ("gpp_weapon_firerate", &["weapon.fireActions[].fireRate"]),
    ("gpp_weapon_spread", &["weapon.fireActions[].spread"]),
    ("gpp_weapon_recoil_kick", &[]),
    ("gpp_weapon_recoil_handling", &[]),
    ("gpp_weapon_recoil_smoothness", &[]),
    ("gpp_weapon_reloadspeed", &[]),
    (
        "gpp_itemresource_powergeneration",
        &["powerPlant.powerOutput"],
    ),
    (
        "gpp_itemresource_coolantgeneration",
        &["cooler.coolingRate"],
    ),
    ("gpp_shield_maxhealth", &["shield.maxHealth.value"]),
    (
        "gpp_radar_minaimassistdistance",
        &["radar.aimAssistDistanceMin"],
    ),
    (
        "gpp_radar_maxaimassistdistance",
        &["radar.aimAssistDistanceMax"],
    ),
    (
        "gpp_quantum_speed",
        &["quantumDrive.standard.value.driveSpeed.value"],
    ),
    (
        "gpp_quantum_fuelrequirement",
        &["quantumDrive.fuelRequirement.value"],
    ),
    ("gpp_weapon_tractor_force", &["tractorBeam.maxForce"]),
    ("gpp_weapon_tractor_maxdist", &["tractorBeam.maxDistance"]),
    (
        "gpp_weapon_tractor_fullstrengthdist",
        &["tractorBeam.fullStrengthDistance"],
    ),
    ("gpp_weapon_tractor_maxvolume", &["tractorBeam.maxVolume"]),
    (
        "gpp_weapon_hullscraping_radius",
        &["weaponModifier.salvage.radiusMultiplier"],
    ),
    (
        "gpp_weapon_hullscraping_speed",
        &["weaponModifier.salvage.salvageSpeedMultiplier"],
    ),
    (
        "gpp_weapon_hullscraping_efficiency",
        &["weaponModifier.salvage.extractionEfficiency"],
    ),
    ("gpp_crafter_craftspeed", &[]),
    ("gpp_crafter_dismantleefficiency", &[]),
];

#[expect(clippy::print_stdout, reason = "the numbers are the result of the run")]
fn report(line: &str) {
    println!("{line}");
}

fn reaches(value: &Value, path: &str) -> bool {
    let Some((head, rest)) = path.split_once('.') else {
        return filled(value, path);
    };
    if let Some(name) = head.strip_suffix("[]") {
        return value
            .get(name)
            .and_then(Value::as_array)
            .is_some_and(|items| items.iter().any(|item| reaches(item, rest)));
    }
    value.get(head).is_some_and(|inner| reaches(inner, rest))
}

fn filled(value: &Value, name: &str) -> bool {
    if let Some(name) = name.strip_suffix("[]") {
        return value
            .get(name)
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty());
    }
    value.get(name).is_some_and(|inner| !inner.is_null())
}

fn source_blocks<'catalog>(
    catalog: &'catalog Value,
    item: &'catalog Value,
    block: &str,
) -> Vec<&'catalog Value> {
    let table = match block {
        "shield" => "/configurations/shieldGenerators/rows",
        "quantumDrive" => "/configurations/quantumDrives/rows",
        _ => {
            return item["stats"]
                .get(block)
                .filter(|value| !value.is_null())
                .into_iter()
                .collect();
        }
    };
    let guid = item.get("guid");
    catalog
        .pointer(table)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|row| guid.is_some() && row.pointer("/ownerGuid/value") == guid)
        .collect()
}

fn item_reaches(catalog: &Value, item: &Value, path: &str) -> bool {
    let (block, fields) = path.split_once('.').unwrap_or((path, ""));
    source_blocks(catalog, item, block)
        .into_iter()
        .any(|source| fields.is_empty() || reaches(source, fields))
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(
    clippy::too_many_lines,
    reason = "one pass over the blueprints that prints its evidence as it goes"
)]
fn every_modified_property_is_filled_on_the_item_the_blueprint_produces() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, None)?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let mut classes = HashMap::new();
    for property in catalog_v3_consumer::rows(&catalog, Kind::GameplayProperties)? {
        let guid = catalog_v3_consumer::identity(property, Kind::GameplayProperties)?;
        let class = catalog_v3_consumer::field(property, Kind::GameplayProperties, "className")?
            .as_str()
            .ok_or("a present property className")?;
        classes.insert(guid, class.to_ascii_lowercase());
    }
    let items: HashMap<&str, &Value> = catalog["items"]
        .as_array()
        .ok_or("the items")?
        .iter()
        .filter_map(|item| Some((item["guid"].as_str()?, item)))
        .collect();
    let mut uses: BTreeMap<String, usize> = BTreeMap::new();
    let mut misses: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut no_source: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut unmapped = 0_usize;
    let blueprints = Blueprints::read(&catalog)?;
    for blueprint in blueprints.rows() {
        let guids: std::collections::BTreeSet<_> = blueprint
            .costs()
            .flat_map(|cost| {
                cost["context"]["modifiers"]
                    .as_array()
                    .into_iter()
                    .flatten()
            })
            .filter_map(|modifier| modifier["propertyGuid"]["value"].as_str())
            .collect();
        let produced = blueprint.presentation["producedItem"]["value"]
            .as_str()
            .and_then(|guid| items.get(guid));
        for guid in guids {
            let class = classes.get(guid).ok_or("an unknown property")?;
            let (_, paths) = FIELDS
                .iter()
                .find(|(name, _)| name == class)
                .ok_or_else(|| format!("{class} is not in the table"))?;
            *uses.entry(class.clone()).or_default() += 1;
            if paths.is_empty() {
                continue;
            }
            let Some(item) = produced else {
                unmapped += 1;
                continue;
            };
            if !paths.iter().any(|path| item_reaches(&catalog, item, path)) {
                let beams_only = item["stats"]["weapon"]["fireActions"]
                    .as_array()
                    .is_some_and(|actions| {
                        actions
                            .iter()
                            .all(|action| action["kind"] == "SWeaponActionFireBeamParams")
                    });
                let without_source = (class == "gpp_weapon_firerate" && beams_only)
                    || paths.iter().all(|path| {
                        let block = path.split('.').next().unwrap_or(path);
                        source_blocks(&catalog, item, block).is_empty()
                    });
                let list = if without_source {
                    &mut no_source
                } else {
                    &mut misses
                };
                list.entry(class.clone())
                    .or_default()
                    .push(item["className"].as_str().unwrap_or("?").to_owned());
            }
        }
    }
    report(&format!("blueprints: {}", blueprints.rows().len()));
    for (class, count) in &uses {
        report(&format!("{class}: {count} blueprints"));
    }
    report(&format!(
        "modifiers of a blueprint without a produced item: {unmapped}"
    ));
    for (class, items) in &no_source {
        report(&format!(
            "NO SOURCE COMPONENT {class}: {} items: {items:?}",
            items.len()
        ));
    }
    for (class, items) in &misses {
        report(&format!(
            "MISS {class}: {} items, e.g. {:?}",
            items.len(),
            items.iter().take(5).collect::<Vec<_>>()
        ));
    }
    assert!(misses.is_empty(), "properties without their stat field");
    Ok(())
}
