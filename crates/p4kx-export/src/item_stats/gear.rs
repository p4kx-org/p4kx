// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataForgeError, RecordTree, StructValue, TreeBuilder};

use super::LinkedStatTarget;
use super::walk::struct_named;
use super::weapon::linked_target;
use crate::contract_stats_gear::{
    ArmorStats, ClothingStats, DamageResistance, RadarStats, ResistanceEntry, SalvageModifier,
    TractorBeamStats, TractorModifier, WeaponModifierStats,
};
use crate::record_fields::{boolean, component, number, structure};

pub(super) const RESISTANCE_MACRO: &str = "DamageResistanceMacro";
pub(super) const ARMOR_COMPONENT: &str = "SCItemSuitArmorParams";
pub(super) const ARMOR_LINK: &str = "damageResistance";
const TRACTOR_ACTION: &str = "SWeaponActionFireTractorBeamParams";
const SALVAGE_MODIFIER: &str = "SSalvageModifier";
const TRACTOR_MODIFIER: &str = "STractorModifier";

fn entry(parent: Option<&StructValue<'_>>, name: &str) -> Option<ResistanceEntry> {
    let entry = structure(parent, name)?;
    let value = |field| number(Some(entry), field);
    Some(ResistanceEntry {
        multiplier: value("Multiplier"),
        threshold: value("Threshold"),
        damage_cap: value("DamageCap"),
        damage_cap_limit: value("DamageCapLimit"),
    })
}

fn resistance(tree: &RecordTree<'_>) -> Option<DamageResistance> {
    let root = structure(Some(tree.root()), "damageResistance")?;
    let resistance = Some(root);
    Some(DamageResistance {
        ignore_melee_damage: boolean(resistance, "IgnoreMeleeDamage"),
        physical: entry(resistance, "PhysicalResistance"),
        energy: entry(resistance, "EnergyResistance"),
        distortion: entry(resistance, "DistortionResistance"),
        thermal: entry(resistance, "ThermalResistance"),
        biochemical: entry(resistance, "BiochemicalResistance"),
        stun: entry(resistance, "StunResistance"),
    })
}

/// Returns the armour block of the item `tree`; `None` without `SCItemSuitArmorParams`.
///
/// The resistances are read from the `DamageResistanceMacro` Record the item links to; one that
/// is missing, has another struct or cannot be built leaves them `None`.
pub(super) fn armor<'f, E>(
    tree: &RecordTree<'f>,
    forge: &'f DataForge<'f>,
    trees: &mut TreeBuilder<'f>,
    observe: impl FnOnce(
        LinkedStatTarget<'_, 'f>,
        Result<&RecordTree<'f>, &DataForgeError>,
    ) -> Result<(), E>,
) -> Result<Option<ArmorStats>, E> {
    let Some(suit) = component(tree, ARMOR_COMPONENT) else {
        return Ok(None);
    };
    let linked = suit
        .field(ARMOR_LINK)
        .and_then(|selector| linked_target(selector, RESISTANCE_MACRO, forge));
    let damage_resistance = if let Some(target) = linked {
        let body = trees.build(&target.record);
        observe(target, body.as_ref())?;
        body.as_ref().ok().and_then(resistance)
    } else {
        None
    };
    Ok(Some(ArmorStats {
        integrity_milestone_to_break: number(Some(suit), "integrityMilestoneToBreak"),
        damage_resistance,
    }))
}

pub(super) fn clothing(tree: &RecordTree<'_>) -> Option<ClothingStats> {
    let clothing = component(tree, "SCItemClothingParams")?;
    let temperature = structure(Some(clothing), "TemperatureResistance");
    let radiation = structure(Some(clothing), "RadiationResistance");
    Some(ClothingStats {
        temperature_resistance_min: number(temperature, "MinResistance"),
        temperature_resistance_max: number(temperature, "MaxResistance"),
        radiation_capacity: number(radiation, "MaximumRadiationCapacity"),
        radiation_dissipation_rate: number(radiation, "RadiationDissipationRate"),
        g_force_resistance: number(structure(Some(clothing), "Flight"), "gForceResistance"),
    })
}

pub(super) fn radar(tree: &RecordTree<'_>) -> Option<RadarStats> {
    let radar = component(tree, "SCItemRadarComponentParams")?;
    let aim = structure(Some(radar), "aimAssist");
    Some(RadarStats {
        aim_assist_distance_min: number(aim, "distanceMinAssignment"),
        aim_assist_distance_max: number(aim, "distanceMaxAssignment"),
        aim_assist_outside_range_buffer: number(aim, "outsideRangeBufferDistance"),
        ping_cooldown_time: number(structure(Some(radar), "pingProperties"), "cooldownTime"),
    })
}

pub(super) fn tractor_beam(tree: &RecordTree<'_>) -> Option<TractorBeamStats> {
    let weapon = component(tree, "SCItemWeaponComponentParams")?;
    let action = Some(struct_named(weapon, TRACTOR_ACTION)?);
    let value = |name| number(action, name);
    Some(TractorBeamStats {
        min_force: value("minForce"),
        max_force: value("maxForce"),
        min_distance: value("minDistance"),
        max_distance: value("maxDistance"),
        full_strength_distance: value("fullStrengthDistance"),
        max_volume: value("maxVolume"),
        max_angle: value("maxAngle"),
    })
}

pub(super) fn weapon_modifier(tree: &RecordTree<'_>) -> Option<WeaponModifierStats> {
    let attachable = component(tree, "EntityComponentAttachableModifierParams")?;
    let salvage = struct_named(attachable, SALVAGE_MODIFIER).map(|modifier| {
        let value = |name| number(Some(modifier), name);
        SalvageModifier {
            radius_multiplier: value("radiusMultiplier"),
            salvage_speed_multiplier: value("salvageSpeedMultiplier"),
            extraction_efficiency: value("extractionEfficiency"),
        }
    });
    let tractor = struct_named(attachable, TRACTOR_MODIFIER).map(|modifier| {
        let value = |name| number(Some(modifier), name);
        TractorModifier {
            force_multiplier: value("forceMultiplier"),
            full_strength_dist_multiplier: value("fullStrengthDistMultiplier"),
            max_dist_multiplier: value("maxDistMultiplier"),
            max_volume_multiplier: value("maxVolumeMultiplier"),
        }
    });
    if salvage.is_none() && tractor.is_none() {
        return None;
    }
    Some(WeaponModifierStats { salvage, tractor })
}
