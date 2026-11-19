// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Record, RecordTree, TreeBuilder, Value as Original, ValueError};

use super::{BuildIdentity, Selection, TestResult};

pub(super) const DAMAGE_RESISTANCE_MACRO: &str = "DamageResistanceMacro";
pub(super) const MINING_LASER_GLOBAL: &str = "MiningLaserGlobalParams";
pub(super) const TURRET_HEALTH_MODIFIER: &str = "STurretHealthModifierDef";
pub(super) const CONSUMABLE_TYPE: &str = "ConsumableType";
pub(super) const CONSUMABLE_SUBTYPE: &str = "ConsumableSubtype";

pub(super) fn discover<'source>(
    forge: &'source DataForge<'source>,
    build: BuildIdentity,
) -> TestResult<Selection> {
    let mut trees = TreeBuilder::new();
    let mut armor = None;
    let mut mining = None;
    let mut turret = None;
    let mut consumable_type = None;
    let mut consumable_subtype = None;
    for record in forge.records() {
        match record.type_name() {
            Ok(DAMAGE_RESISTANCE_MACRO) => {
                select(&mut armor, &record, &mut trees, has_resistance);
            }
            Ok(MINING_LASER_GLOBAL) => {
                select(&mut mining, &record, &mut trees, has_throttle);
            }
            Ok(TURRET_HEALTH_MODIFIER) => {
                select(&mut turret, &record, &mut trees, has_curve);
            }
            Ok(CONSUMABLE_TYPE) => {
                select(&mut consumable_type, &record, &mut trees, has_subtype_links);
            }
            Ok(CONSUMABLE_SUBTYPE) => {
                select(
                    &mut consumable_subtype,
                    &record,
                    &mut trees,
                    has_numeric_effect,
                );
            }
            _ => continue,
        }
        if [armor, mining, turret, consumable_type, consumable_subtype]
            .iter()
            .all(Option::is_some)
        {
            break;
        }
    }
    Ok(Selection {
        build,
        damage_resistance_macro: armor
            .ok_or("no readable DamageResistanceMacro multiplier witness")?,
        mining_laser_global: mining
            .ok_or("no readable MiningLaserGlobalParams throttle witness")?,
        turret_health_modifier: turret
            .ok_or("no readable STurretHealthModifierDef curve witness")?,
        consumable_type: consumable_type
            .ok_or("no readable ConsumableType subtype link witness")?,
        consumable_subtype: consumable_subtype
            .ok_or("no readable ConsumableSubtype numeric effect witness")?,
    })
}

fn select<'source>(
    selected: &mut Option<u32>,
    record: &Record<'source>,
    trees: &mut TreeBuilder<'source>,
    positive: impl FnOnce(&RecordTree<'source>) -> bool,
) {
    if selected.is_none()
        && let Ok(tree) = trees.build(record)
        && positive(&tree)
    {
        *selected = Some(record.index());
    }
}

const fn single(value: Option<&Original<'_>>) -> bool {
    matches!(value, Some(Original::Single(_)))
}

fn has_resistance(tree: &RecordTree<'_>) -> bool {
    let Some(Original::Struct(body)) = tree.field("damageResistance") else {
        return false;
    };
    [
        "PhysicalResistance",
        "EnergyResistance",
        "DistortionResistance",
        "ThermalResistance",
        "BiochemicalResistance",
        "StunResistance",
    ]
    .iter()
    .any(|field| match body.field(field) {
        Some(Original::Struct(channel)) => single(channel.field("Multiplier")),
        _ => false,
    })
}

fn has_throttle(tree: &RecordTree<'_>) -> bool {
    [
        "throttleChangePerAction",
        "throttleAccPeriod",
        "throttleAccFactor",
        "throttleHoldAccFactor",
    ]
    .iter()
    .any(|field| single(tree.field(field)))
}

fn has_curve(tree: &RecordTree<'_>) -> bool {
    let Some(Original::Struct(curve)) = tree.field("damageMovementModifier") else {
        return false;
    };
    let Some(Original::Array(points)) = curve.field("points") else {
        return false;
    };
    points.iter().any(|point| match point {
        Original::Struct(point) => single(point.field("x")) && single(point.field("y")),
        _ => false,
    })
}

fn has_subtype_links(tree: &RecordTree<'_>) -> bool {
    let Some(Original::Array(slots)) = tree.field("subtypes") else {
        return false;
    };
    let mut resolved = false;
    for slot in slots {
        match slot {
            Original::Reference(target) if target.type_name() == CONSUMABLE_SUBTYPE => {
                resolved = true;
            }
            Original::Null | Original::Error(ValueError::DanglingReference { .. }) => {}
            _ => return false,
        }
    }
    resolved
}

fn has_numeric_effect(tree: &RecordTree<'_>) -> bool {
    let Some(Original::Array(effects)) = tree.field("effectsPerMicroSCU") else {
        return false;
    };
    effects.iter().any(|effect| {
        let Original::Struct(effect) = effect else {
            return false;
        };
        match effect.type_name() {
            "ConsumableEffectHealth" => single(effect.field("healthChange")),
            "ConsumableEffectResource" => single(effect.field("amount")),
            "ConsumableEffectModifyActorStatus" => {
                single(effect.field("statPointChange"))
                    || single(effect.field("statCooldownChange"))
            }
            _ => false,
        }
    })
}
