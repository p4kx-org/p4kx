// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Serialize;

/// The suit armour of an item, raw facts of `SCItemSuitArmorParams` (ADR-0072).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArmorStats {
    /// `integrityMilestoneToBreak`.
    pub(crate) integrity_milestone_to_break: Option<f32>,
    /// The `damageResistance` of the `DamageResistanceMacro` Record that `damageResistance` links
    /// to; `null` without a readable one.
    pub(crate) damage_resistance: Option<DamageResistance>,
}

/// The resistances of a `DamageResistanceMacro`, one entry per damage type.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DamageResistance {
    /// `IgnoreMeleeDamage`.
    pub(crate) ignore_melee_damage: Option<bool>,
    /// `PhysicalResistance`.
    pub(crate) physical: Option<ResistanceEntry>,
    /// `EnergyResistance`.
    pub(crate) energy: Option<ResistanceEntry>,
    /// `DistortionResistance`.
    pub(crate) distortion: Option<ResistanceEntry>,
    /// `ThermalResistance`.
    pub(crate) thermal: Option<ResistanceEntry>,
    /// `BiochemicalResistance`.
    pub(crate) biochemical: Option<ResistanceEntry>,
    /// `StunResistance`.
    pub(crate) stun: Option<ResistanceEntry>,
}

/// The resistance to one damage type, as stored: the multiplier is the factor damage is scaled
/// by, not a mitigation.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResistanceEntry {
    /// `Multiplier`.
    pub(crate) multiplier: Option<f32>,
    /// `Threshold`.
    pub(crate) threshold: Option<f32>,
    /// `DamageCap`.
    pub(crate) damage_cap: Option<f32>,
    /// `DamageCapLimit`.
    pub(crate) damage_cap_limit: Option<f32>,
}

/// The worn properties of an item, raw facts of `SCItemClothingParams`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClothingStats {
    /// `TemperatureResistance.MinResistance`, in degrees Celsius.
    pub(crate) temperature_resistance_min: Option<f32>,
    /// `TemperatureResistance.MaxResistance`, in degrees Celsius.
    pub(crate) temperature_resistance_max: Option<f32>,
    /// `RadiationResistance.MaximumRadiationCapacity`.
    pub(crate) radiation_capacity: Option<f32>,
    /// `RadiationResistance.RadiationDissipationRate`.
    pub(crate) radiation_dissipation_rate: Option<f32>,
    /// `Flight.gForceResistance`.
    pub(crate) g_force_resistance: Option<f32>,
}

/// A radar, raw facts of `SCItemRadarComponentParams`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RadarStats {
    /// `aimAssist.distanceMinAssignment`, in metres.
    pub(crate) aim_assist_distance_min: Option<f32>,
    /// `aimAssist.distanceMaxAssignment`, in metres.
    pub(crate) aim_assist_distance_max: Option<f32>,
    /// `aimAssist.outsideRangeBufferDistance`, in metres.
    pub(crate) aim_assist_outside_range_buffer: Option<f32>,
    /// `pingProperties.cooldownTime`, in seconds.
    pub(crate) ping_cooldown_time: Option<f32>,
}

/// A tractor beam, from its `SWeaponActionFireTractorBeamParams`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TractorBeamStats {
    /// `minForce`.
    pub(crate) min_force: Option<f32>,
    /// `maxForce`.
    pub(crate) max_force: Option<f32>,
    /// `minDistance`, in metres.
    pub(crate) min_distance: Option<f32>,
    /// `maxDistance`, in metres.
    pub(crate) max_distance: Option<f32>,
    /// `fullStrengthDistance`, in metres.
    pub(crate) full_strength_distance: Option<f32>,
    /// `maxVolume`.
    pub(crate) max_volume: Option<f32>,
    /// `maxAngle`, in degrees.
    pub(crate) max_angle: Option<f32>,
}

/// The tractor and salvage multipliers an attachment applies, from the `weaponStats` of its
/// `EntityComponentAttachableModifierParams`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WeaponModifierStats {
    /// The `salvageModifier`; `null` without one.
    pub(crate) salvage: Option<SalvageModifier>,
    /// The `tractorModifier`; `null` without one.
    pub(crate) tractor: Option<TractorModifier>,
}

/// An `SSalvageModifier`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SalvageModifier {
    /// `radiusMultiplier`.
    pub(crate) radius_multiplier: Option<f32>,
    /// `salvageSpeedMultiplier`.
    pub(crate) salvage_speed_multiplier: Option<f32>,
    /// `extractionEfficiency`.
    pub(crate) extraction_efficiency: Option<f32>,
}

/// An `STractorModifier`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[expect(
    clippy::struct_field_names,
    reason = "each field is named after the stored multiplier"
)]
pub(crate) struct TractorModifier {
    /// `forceMultiplier`.
    pub(crate) force_multiplier: Option<f32>,
    /// `fullStrengthDistMultiplier`.
    pub(crate) full_strength_dist_multiplier: Option<f32>,
    /// `maxDistMultiplier`.
    pub(crate) max_dist_multiplier: Option<f32>,
    /// `maxVolumeMultiplier`.
    pub(crate) max_volume_multiplier: Option<f32>,
}
