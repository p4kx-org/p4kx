// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::real_archive_equipment_catalog::{
    TestResult, WEAPONS, action_nodes, bullet_damage, catalog, item, item_catalog, owner,
    owner_rows, primary_ammo,
};
use crate::real_archive_item_durability::rows;
use crate::real_archive_resource_components::known_generation_amount;
use crate::real_archive_verification::print_line;

const MISSILES: &str = "/configurations/missileConfigurations/rows";
const SHIELDS: &str = "/configurations/shieldGenerators/rows";
const EXPLOSIVES: &str = "/configurations/explosives/rows";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn every_component_table_covers_its_authored_item_family() -> TestResult {
    let catalog = catalog()?;
    let items = rows(&catalog, "/items")?;
    let weapons: Vec<_> = items
        .iter()
        .filter(|item| owner_rows(&catalog, WEAPONS, item).next().is_some())
        .collect();
    check_share(
        "weapon fire actions",
        weapons
            .iter()
            .filter(|item| {
                owner_rows(&catalog, WEAPONS, item).any(|row| action_nodes(row).next().is_some())
            })
            .count(),
        weapons.len(),
    );
    check_share(
        "weapon primary capacity",
        weapons
            .iter()
            .filter(|item| {
                primary_ammo(&catalog, item)
                    .is_some_and(|ammo| ammo.container["maxAmmoCount"]["value"].is_number())
            })
            .count(),
        weapons.len(),
    );
    check_share(
        "weapon ammunition definitions",
        weapons
            .iter()
            .filter(|item| {
                primary_ammo(&catalog, item).is_some_and(|ammo| ammo.definition.is_some())
            })
            .count(),
        weapons.len(),
    );
    validate_missile_and_shield_coverage(&catalog)
}

fn validate_missile_and_shield_coverage(catalog: &Value) -> TestResult {
    let missiles = rows(catalog, MISSILES)?;
    let explosives = rows(catalog, EXPLOSIVES)?;
    let ordnance: Vec<_> = explosives
        .iter()
        .filter(|row| {
            row.pointer("/configuration/value/type")
                .and_then(Value::as_str)
                == Some("missile")
        })
        .collect();
    for (label, path) in [
        (
            "missile damage",
            "/configuration/value/value/explosionParams/value/damage/value",
        ),
        (
            "missile maximum radius",
            "/configuration/value/value/explosionParams/value/maxRadius/value/finite",
        ),
    ] {
        check_share(
            label,
            ordnance.iter().filter(|row| present(row, path)).count(),
            missiles.len(),
        );
    }
    for (label, path) in [
        (
            "missile lock time",
            "/targeting/value/lockTime/value/finite",
        ),
        ("missile speed", "/guidance/value/linearSpeed/value/finite"),
    ] {
        check_share(
            label,
            missiles.iter().filter(|row| present(row, path)).count(),
            missiles.len(),
        );
    }
    let shields = rows(catalog, SHIELDS)?;
    for field in [
        "maxHealth",
        "maxRegen",
        "downedRegenDelay",
        "damagedRegenDelay",
    ] {
        let path = format!("/{field}/value/finite");
        check_share(
            field,
            shields.iter().filter(|row| present(row, &path)).count(),
            shields.len(),
        );
    }
    Ok(())
}

fn check_share(label: &str, filled: usize, total: usize) {
    print_line(&format!("{label}: {filled} of {total}"));
    assert!(
        total > 0 && filled.saturating_mul(5) >= total.saturating_mul(4),
        "{label} covers at least 80 percent of its original family"
    );
}

fn present(row: &Value, path: &str) -> bool {
    row.pointer(path).is_some_and(|value| !value.is_null())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn known_items_carry_their_known_values() -> TestResult {
    for (class, table, field, expected) in [
        (
            "SHLD_GODI_S01_FR66_SCItem",
            SHIELDS,
            "/maxHealth/value/finite",
            Value::from(3168.0),
        ),
        (
            "MISL_S07_IR_TALN_Hellion",
            MISSILES,
            "/targeting/value/trackingSignalType/value",
            Value::from("Infrared"),
        ),
    ] {
        let current = item_catalog(class)?;
        let row = owner(&current, table, item(&current, class)?)?;
        assert_eq!(
            row.pointer(field),
            Some(&expected),
            "the stored original component field"
        );
    }
    for (class, resource, amount) in [
        ("COOL_JUST_S02_Snowpack_SCItem", "Coolant", 46.0),
        ("POWR_JUST_S03_Durango_SCItem", "Power", 24.0),
    ] {
        let current = item_catalog(class)?;
        assert_eq!(
            known_generation_amount(&current, item(&current, class)?, resource),
            Some(amount),
            "stored generation in its own original unit"
        );
    }
    known_weapon_and_quantum_drive()
}

fn known_weapon_and_quantum_drive() -> TestResult {
    const WEAPON: &str = "VNCL_LaserCannon_S1";
    const QUANTUM: &str = "QDRV_WETK_S04_Idris_TEMP";
    let current = item_catalog(WEAPON)?;
    let weapon = owner(&current, WEAPONS, item(&current, WEAPON)?)?;
    let first = action_nodes(weapon)
        .next()
        .ok_or("first original fire action")?;
    assert_eq!(
        first
            .pointer("/kind/value/fireRate/value")
            .and_then(Value::as_i64),
        Some(350),
        "the original first action fire rate"
    );
    let current = item_catalog(QUANTUM)?;
    let drive = owner(
        &current,
        "/configurations/quantumDrives/rows",
        item(&current, QUANTUM)?,
    )?;
    assert!(
        drive
            .pointer("/jumpRange/value/finite")
            .and_then(Value::as_f64)
            .is_some_and(|range| range > 3.4e38),
        "the stored f32::MAX"
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn personal_weapons_take_ammunition_from_their_magazine() -> TestResult {
    let catalog = catalog()?;
    let personal: Vec<_> = rows(&catalog, "/items")?
        .iter()
        .filter(|item| item["type"] == "WeaponPersonal")
        .filter(|item| owner_rows(&catalog, WEAPONS, item).next().is_some())
        .collect();
    let magazines = personal
        .iter()
        .filter(|item| {
            primary_ammo(&catalog, item).is_some_and(|ammo| {
                ammo.definition.and_then(bullet_damage).is_some()
                    && ammo
                        .container
                        .pointer("/maxAmmoCount/value")
                        .and_then(Value::as_i64)
                        .is_some_and(|count| count > 0)
            })
        })
        .count();
    check_share(
        "personal weapons with magazine ammunition",
        magazines,
        personal.len(),
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn weapons_state_ammunition_damage_and_a_fire_rate() -> TestResult {
    let catalog = catalog()?;
    let damaging = rows(&catalog, "/items")?
        .iter()
        .filter(|item| {
            primary_ammo(&catalog, item)
                .and_then(|ammo| ammo.definition)
                .and_then(bullet_damage)
                .is_some()
        })
        .count();
    let rated = rows(&catalog, WEAPONS)?
        .iter()
        .filter(|weapon| {
            action_nodes(weapon).any(|node| present(node, "/kind/value/fireRate/value"))
        })
        .count();
    print_line(&format!(
        "weapons with ammunition damage: {damaging}, with a fire rate: {rated}"
    ));
    assert!(damaging > 0, "a weapon with original ammunition damage");
    assert!(rated > 0, "a weapon with an original fire rate");
    Ok(())
}
