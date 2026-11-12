// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::contract_stats_gear::{
    ArmorStats, ClothingStats, RadarStats, TractorBeamStats, WeaponModifierStats,
};
use crate::contract_stats_mining::{MiningLaserStats, MiningModifierStats};

/// Item stats in the compact order used by variant identity (ADR-0356).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ItemStats<'a> {
    pub(crate) weapon: Option<WeaponStats<'a>>,
    pub(crate) shield: Option<ShieldStats>,
    pub(crate) quantum_drive: Option<QuantumDriveStats>,
    #[serde(flatten)]
    pub(crate) fields: ItemStatsFields<'a>,
}

/// Published item stat blocks and health.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "ItemStats", transform = every_field_required)]
pub(crate) struct PublishedItemStats<'a> {
    /// The weapon block, from `SCItemWeaponComponentParams`.
    weapon: Option<WeaponStats<'a>>,
    #[serde(flatten)]
    fields: ItemStatBlocks<'a>,
}

/// Remaining private variant identity fields in compact serialization order.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ItemStatsFields<'a> {
    pub(crate) power_plant: Option<PowerPlantStats<'a>>,
    pub(crate) cooler: Option<CoolerStats<'a>>,
    #[serde(flatten)]
    pub(crate) blocks: ItemStatBlocks<'a>,
    pub(crate) resource_states: Vec<ResourceState<'a>>,
}

/// Shared specialist blocks in their public serialization order.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ItemStatBlocks<'a> {
    /// The missile block, from `SCItemMissileParams`.
    pub(crate) missile: Option<MissileStats<'a>>,
    /// The suit armour block, from `SCItemSuitArmorParams`.
    pub(crate) armor: Option<ArmorStats>,
    /// The worn properties block, from `SCItemClothingParams`.
    pub(crate) clothing: Option<ClothingStats>,
    /// The radar block, from `SCItemRadarComponentParams`.
    pub(crate) radar: Option<RadarStats>,
    /// The tractor beam block, from the `SWeaponActionFireTractorBeamParams` of the weapon.
    pub(crate) tractor_beam: Option<TractorBeamStats>,
    /// The tractor and salvage multipliers of an attachment, from
    /// `EntityComponentAttachableModifierParams`.
    pub(crate) weapon_modifier: Option<WeaponModifierStats>,
    /// The mining laser block, from `SEntityComponentMiningLaserParams`: ship lasers and the FPS
    /// mining tool.
    pub(crate) mining_laser: Option<MiningLaserStats>,
    /// The mining module or gadget block, from the `EntityComponentAttachableModifierParams` that
    /// holds a mining modifier.
    pub(crate) mining_modifier: Option<MiningModifierStats<'a>>,
    /// The hit points, from `SHealthComponentParams.Health`.
    pub(crate) health: Option<f32>,
}

impl<'a> ItemStats<'a> {
    pub(crate) const fn has_specialist(&self) -> bool {
        self.weapon.is_some()
            || self.shield.is_some()
            || self.quantum_drive.is_some()
            || self.fields.has_specialist()
    }

    pub(crate) fn into_published(self) -> Option<PublishedItemStats<'a>> {
        let Self { weapon, fields, .. } = self;
        let fields = fields.blocks;
        (weapon.is_some() || fields.has_specialist() || fields.health.is_some())
            .then_some(PublishedItemStats { weapon, fields })
    }
}

impl ItemStatsFields<'_> {
    const fn has_specialist(&self) -> bool {
        self.power_plant.is_some() || self.cooler.is_some() || self.blocks.has_specialist()
    }
}

impl ItemStatBlocks<'_> {
    const fn has_specialist(&self) -> bool {
        self.missile.is_some()
            || self.armor.is_some()
            || self.clothing.is_some()
            || self.radar.is_some()
            || self.tractor_beam.is_some()
            || self.weapon_modifier.is_some()
            || self.mining_laser.is_some()
            || self.mining_modifier.is_some()
    }
}

/// One state of the resource component: its signatures and the resources it generates and uses.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ResourceState<'a> {
    /// The `name` of the state, e.g. `Online`.
    pub(crate) name: Option<&'a str>,
    /// `signatureParams.EMSignature`.
    pub(crate) em_signature: Option<Signature>,
    /// `signatureParams.IRSignature`.
    pub(crate) ir_signature: Option<Signature>,
    /// The `generation` and `consumption` of the state's `deltas`, in stored order.
    pub(crate) flows: Vec<ResourceFlow<'a>>,
}

/// A signature of a state.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Signature {
    /// `nominalSignature`.
    pub(crate) nominal: Option<f32>,
    /// `decayRate`.
    pub(crate) decay_rate: Option<f32>,
}

/// One generation or consumption of a resource, per second, in the unit the Archive states.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ResourceFlow<'a> {
    /// The struct of the delta, e.g. `ItemResourceDeltaGeneration`. A closed set the game may
    /// extend; the schema keeps a string.
    pub(crate) kind: &'a str,
    /// `generation` or `consumption`: the field of the delta that holds the flow.
    pub(crate) direction: &'a str,
    /// The `resource`, e.g. `Power`. A closed set the game may extend; the schema keeps a string.
    pub(crate) resource: Option<&'a str>,
    /// The first number of `resourceAmountPerSecond`, in the unit named by `unit`.
    pub(crate) amount: Option<f32>,
    /// The struct that holds the amount, e.g. `SPowerSegmentResourceUnit`; the units of two
    /// structs differ and are not converted.
    pub(crate) unit: Option<&'a str>,
}

/// A weapon: its fire modes and the ammunition it fires.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct WeaponStats<'a> {
    /// The fire modes of `fireActions` in stored order; a sequence is flattened to its entries.
    pub(crate) fire_actions: Vec<FireAction<'a>>,
    /// `maxAmmoCount` of the ammo container: the weapon's own `SAmmoContainerComponentParams` or,
    /// for a weapon without one, that of the magazine entity `ammoContainerRecord` links to. The
    /// capacity of the magazine, not of the weapon body.
    pub(crate) ammo_capacity: Option<i64>,
    /// The ammunition, from the `AmmoParams` Record that `ammoParamsRecord` of that container
    /// links to; `null` without a readable one.
    pub(crate) ammo: Option<AmmoStats>,
}

/// One fire mode of a weapon.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct FireAction<'a> {
    /// The struct of the fire mode, e.g. `SWeaponActionFireRapidParams`. A closed set the game
    /// may extend; the schema keeps a string.
    pub(crate) kind: &'a str,
    /// The `name` of the fire mode.
    pub(crate) name: Option<&'a str>,
    /// `fireRate`, in rounds per minute. `fireRate`, `heatPerShot`, `chargeTime`,
    /// `overchargeTime`, `cooldownTime`, `name` and `launchParams` are read from the fire mode and,
    /// where it has none of its own, from its `weaponAction`.
    pub(crate) fire_rate: Option<f32>,
    /// `launchParams.pelletCount` (`pellets` where a Record states that instead), the projectiles
    /// of one shot.
    pub(crate) pellets: Option<i64>,
    /// `launchParams.ammoCost`, the rounds one shot uses.
    pub(crate) ammo_cost: Option<i64>,
    /// `heatPerShot`.
    pub(crate) heat_per_shot: Option<f32>,
    /// `chargeTime` of the fire mode, when stated, in seconds.
    pub(crate) charge_time: Option<f32>,
    /// `overchargeTime` of the fire mode, when stated, in seconds.
    pub(crate) overcharge_time: Option<f32>,
    /// `cooldownTime` of the fire mode, when stated, in seconds.
    pub(crate) cooldown_time: Option<f32>,
    /// `hitType` of a beam, e.g. `ElectricArc` for a mining beam and `Extraction` for an
    /// extraction beam. A closed set the game may extend; the schema keeps a string.
    pub(crate) hit_type: Option<&'a str>,
    /// `fullDamageRange` of a beam, in metres: the distance up to which it does full damage.
    pub(crate) full_damage_range: Option<f32>,
    /// `zeroDamageRange` of a beam, in metres: the distance from which it does no damage.
    pub(crate) zero_damage_range: Option<f32>,
    /// `damagePerSecond`, the damage a beam does each second; `null` for a mode without one.
    pub(crate) damage_per_second: Option<Damage>,
    /// `launchParams.spreadParams`; `null` for a mode without one.
    pub(crate) spread: Option<Spread>,
}

/// The spread of a fire mode, as stored in `SSpreadParams`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Spread {
    /// `min`.
    pub(crate) min: Option<f32>,
    /// `max`.
    pub(crate) max: Option<f32>,
    /// `firstAttack`.
    pub(crate) first_attack: Option<f32>,
    /// `attack`.
    pub(crate) attack: Option<f32>,
    /// `decay`.
    pub(crate) decay: Option<f32>,
}

/// The ammunition of a weapon.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AmmoStats {
    /// `speed`, in metres per second.
    pub(crate) speed: Option<f32>,
    /// `lifetime`, in seconds.
    pub(crate) lifetime: Option<f32>,
    /// The damage on impact, from `projectileParams.damage`.
    pub(crate) damage: Option<Damage>,
    /// The damage of the detonation, from the `damage` of `explosionParams`.
    pub(crate) detonation_damage: Option<Damage>,
    /// `explosionParams.minRadius`, in metres.
    pub(crate) detonation_min_radius: Option<f32>,
    /// `explosionParams.maxRadius`, in metres.
    pub(crate) detonation_max_radius: Option<f32>,
}

/// Damage by type, as the Archive states it.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Damage {
    /// `DamagePhysical`.
    pub(crate) physical: Option<f32>,
    /// `DamageEnergy`.
    pub(crate) energy: Option<f32>,
    /// `DamageDistortion`.
    pub(crate) distortion: Option<f32>,
    /// `DamageThermal`.
    pub(crate) thermal: Option<f32>,
    /// `DamageBiochemical`.
    pub(crate) biochemical: Option<f32>,
    /// `DamageStun`.
    pub(crate) stun: Option<f32>,
}

/// The shield block used by item variant identity.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShieldStats {
    /// `MaxShieldHealth`.
    pub(crate) max_health: Option<f32>,
    /// `MaxShieldRegen`, in hit points per second.
    pub(crate) max_regen: Option<f32>,
    /// `DecayRatio`.
    pub(crate) decay_ratio: Option<f32>,
    /// `DownedRegenDelay`, in seconds.
    pub(crate) downed_regen_delay: Option<f32>,
    /// `DamagedRegenDelay`, in seconds.
    pub(crate) damaged_regen_delay: Option<f32>,
}

/// The compact quantum values consumed by variant identity.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuantumDriveStats {
    /// `quantumFuelRequirement`, as stored.
    pub(crate) fuel_requirement: Option<f32>,
    /// `jumpRange`, in metres.
    pub(crate) jump_range: Option<f32>,
    /// `disconnectRange`, in metres.
    pub(crate) disconnect_range: Option<f32>,
    /// `params.driveSpeed`, in metres per second.
    pub(crate) drive_speed: Option<f32>,
    /// `params.cooldownTime`, in seconds.
    pub(crate) cooldown_time: Option<f32>,
    /// `params.spoolUpTime`, in seconds.
    pub(crate) spool_up_time: Option<f32>,
    /// `params.stageOneAccelRate`.
    pub(crate) stage_one_accel_rate: Option<f32>,
    /// `params.stageTwoAccelRate`.
    pub(crate) stage_two_accel_rate: Option<f32>,
    /// `params.engageSpeed`, in metres per second.
    pub(crate) engage_speed: Option<f32>,
}

/// A power plant.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PowerPlantStats<'a> {
    /// The `amount` of the first `generation` flow of `resourceStates` whose resource is `Power`.
    pub(crate) power_output: Option<f32>,
    /// The `unit` of that flow.
    pub(crate) unit: Option<&'a str>,
}

/// A cooler.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CoolerStats<'a> {
    /// The `amount` of the first `generation` flow of `resourceStates` whose resource is
    /// `Coolant`.
    pub(crate) cooling_rate: Option<f32>,
    /// The `unit` of that flow.
    pub(crate) unit: Option<&'a str>,
}

/// A missile.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissileStats<'a> {
    /// The damage of the explosion, from the `damage` of `explosionParams`.
    pub(crate) damage: Option<Damage>,
    /// `explosionParams.minRadius`, in metres.
    pub(crate) explosion_min_radius: Option<f32>,
    /// `explosionParams.maxRadius`, in metres.
    pub(crate) explosion_max_radius: Option<f32>,
    /// `targetingParams.trackingSignalType`. A closed set the game may extend; the schema keeps a
    /// string.
    pub(crate) tracking_signal_type: Option<&'a str>,
    /// `targetingParams.lockTime`, in seconds.
    pub(crate) lock_time: Option<f32>,
    /// `targetingParams.lockingAngle`, in degrees.
    pub(crate) locking_angle: Option<f32>,
    /// `targetingParams.lockRangeMin`, in metres.
    pub(crate) lock_range_min: Option<f32>,
    /// `targetingParams.lockRangeMax`, in metres.
    pub(crate) lock_range_max: Option<f32>,
    /// `GCSParams.linearSpeed`, in metres per second.
    pub(crate) linear_speed: Option<f32>,
}
