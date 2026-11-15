// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Every gameplay property a blueprint modifies finds its stat field on the item the blueprint
//! produces, on a real Archive; runs with `--ignored` when `P4KX_DATA_P4K` names one.

use p4kx_export::Kind;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

use crate::catalog_v3_consumer;
use crate::real_archive_blueprints::Blueprints;

use crate::real_archive_equipment_catalog::{
    TestResult, WEAPONS, action_nodes, catalog, definition, owner_rows, primary_ammo,
};

const FIELDS: [(&str, &[&str]); 29] = [
    (
        "gpp_armor_damagemitigation",
        &["armorMacro.damageResistance.value.physical.value.multiplier.value"],
    ),
    (
        "gpp_armor_temperaturemin",
        &["clothing.temperatureResistance.value.minResistance.value"],
    ),
    (
        "gpp_armor_temperaturemax",
        &["clothing.temperatureResistance.value.maxResistance.value"],
    ),
    (
        "gpp_armor_radiationcapacity",
        &["clothing.radiationResistance.value.maximumRadiationCapacity.value"],
    ),
    (
        "gpp_armor_radiationdissipation",
        &["clothing.radiationResistance.value.radiationDissipationRate.value"],
    ),
    ("gpp_health_maxhealth", &["health.health.value"]),
    (
        "gpp_weapon_damage",
        &[
            "ammo.projectileParams.value.kind.value.damage.value",
            "weapon.actionNodes[].kind.value.damagePerSecond.value.value",
        ],
    ),
    (
        "gpp_weapon_firerate",
        &["weapon.actionNodes[].kind.value.fireRate.value"],
    ),
    (
        "gpp_weapon_spread",
        &["weapon.actionNodes[].kind.value.launchParams.value.spreadParams.value"],
    ),
    ("gpp_weapon_recoil_kick", &[]),
    ("gpp_weapon_recoil_handling", &[]),
    ("gpp_weapon_recoil_smoothness", &[]),
    ("gpp_weapon_reloadspeed", &[]),
    (
        "gpp_itemresource_powergeneration",
        &["resourceGeneration.Power"],
    ),
    (
        "gpp_itemresource_coolantgeneration",
        &["resourceGeneration.Coolant"],
    ),
    ("gpp_shield_maxhealth", &["shield.maxHealth.value"]),
    (
        "gpp_radar_minaimassistdistance",
        &["radar.aimAssist.value.distanceMinAssignment.value"],
    ),
    (
        "gpp_radar_maxaimassistdistance",
        &["radar.aimAssist.value.distanceMaxAssignment.value"],
    ),
    (
        "gpp_quantum_speed",
        &["quantumDrive.standard.value.driveSpeed.value"],
    ),
    (
        "gpp_quantum_fuelrequirement",
        &["quantumDrive.fuelRequirement.value"],
    ),
    ("gpp_weapon_tractor_force", &["tractor.maxForce.value"]),
    ("gpp_weapon_tractor_maxdist", &["tractor.maxDistance.value"]),
    (
        "gpp_weapon_tractor_fullstrengthdist",
        &["tractor.fullStrengthDistance.value"],
    ),
    ("gpp_weapon_tractor_maxvolume", &["tractor.maxVolume.value"]),
    (
        "gpp_weapon_hullscraping_radius",
        &[
            "attachment.modifiers.value[].value.value.weaponModifier.value.weaponStats.value.salvageModifier.value.radiusMultiplier.value",
        ],
    ),
    (
        "gpp_weapon_hullscraping_speed",
        &[
            "attachment.modifiers.value[].value.value.weaponModifier.value.weaponStats.value.salvageModifier.value.salvageSpeedMultiplier.value",
        ],
    ),
    (
        "gpp_weapon_hullscraping_efficiency",
        &[
            "attachment.modifiers.value[].value.value.weaponModifier.value.weaponStats.value.salvageModifier.value.extractionEfficiency.value",
        ],
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
    item: &Value,
    block: &str,
) -> Vec<&'catalog Value> {
    let table = match block {
        "health" => "/configurations/durability/rows",
        "shield" => "/configurations/shieldGenerators/rows",
        "quantumDrive" => "/configurations/quantumDrives/rows",
        "resourceGeneration" => "/configurations/resourceComponents/rows",
        "clothing" => "/configurations/clothing/rows",
        "radar" => "/configurations/itemRadars/rows",
        "weapon" => WEAPONS,
        "attachment" => "/configurations/attachableModifiers/rows",
        "tractor" => {
            return owner_rows(catalog, WEAPONS, item)
                .flat_map(action_nodes)
                .filter(|action| {
                    action.pointer("/kind/type").and_then(Value::as_str) == Some("tractorBeam")
                })
                .filter_map(|action| action.pointer("/kind/value"))
                .collect();
        }
        "armorMacro" => {
            return owner_rows(catalog, "/configurations/suitArmor/rows", item)
                .filter_map(|row| row.pointer("/damageResistance/value/targetRecordIndex/value"))
                .filter_map(|target| {
                    definition(
                        catalog,
                        "/definitions/damageResistanceMacros/rows",
                        target,
                        "/source/recordIndex/value",
                    )
                })
                .collect();
        }
        "ammo" => {
            return primary_ammo(catalog, item)
                .and_then(|ammo| ammo.definition)
                .into_iter()
                .collect();
        }
        _ => return Vec::new(),
    };
    owner_rows(catalog, table, item).collect()
}

fn uses_only_beams(catalog: &Value, item: &Value) -> bool {
    let mut firing = owner_rows(catalog, WEAPONS, item)
        .flat_map(action_nodes)
        .filter_map(|node| node.pointer("/kind/type").and_then(Value::as_str))
        .filter(|kind| matches!(*kind, "beam" | "single" | "rapid" | "burst" | "tractorBeam"));
    firing.next() == Some("beam") && firing.all(|kind| kind == "beam")
}

fn item_reaches(catalog: &Value, item: &Value, path: &str) -> bool {
    let (block, fields) = path.split_once('.').unwrap_or((path, ""));
    if block == "resourceGeneration" {
        return crate::real_archive_resource_components::known_generation_amount(
            catalog, item, fields,
        )
        .is_some();
    }
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
    let catalog = catalog()?;
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
                let beams_only = uses_only_beams(&catalog, item);
                let without_source = (class == "gpp_weapon_firerate" && beams_only)
                    || paths.iter().all(|path| {
                        let block = path.split('.').next().unwrap_or(path);
                        if block == "armorMacro" {
                            owner_rows(&catalog, "/configurations/suitArmor/rows", item)
                                .next()
                                .is_none()
                        } else {
                            source_blocks(&catalog, item, block).is_empty()
                        }
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
