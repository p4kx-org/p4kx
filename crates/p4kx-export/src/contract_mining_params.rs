// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;

/// The mining parameters of one kind of miner: a `MiningGlobalParams` Record under `mining`. Each
/// number is that of the Record, unchanged, and `null` when the Record states none. The effects,
/// sounds and the default explosion of the Record are not exported.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MiningGlobalParams<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `miningglobalparamsship`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// `absorbableVolumeThreshold`.
    pub(crate) absorbable_volume_threshold: Option<f32>,
    /// `cSCUPerVolume`.
    pub(crate) c_scu_per_volume: Option<f32>,
    /// `childRockInactivityLifetime` in seconds.
    pub(crate) child_rock_inactivity_lifetime: Option<f32>,
    /// `childRockInvulnerabilityTime` in seconds.
    pub(crate) child_rock_invulnerability_time: Option<f32>,
    /// `childRockLifeTimer` in seconds.
    pub(crate) child_rock_life_timer: Option<f32>,
    /// `childRockZeroGDamping`.
    pub(crate) child_rock_zero_g_damping: Option<f32>,
    /// `compositionMassMultiplier`; `null` for a Record without it.
    pub(crate) composition_mass_multiplier: Option<f32>,
    /// `controlledBreakingDecayRate`.
    pub(crate) controlled_breaking_decay_rate: Option<f32>,
    /// `controlledBreakingFillRate`.
    pub(crate) controlled_breaking_fill_rate: Option<f32>,
    /// `controlledBreakingFillRateDanger`.
    pub(crate) controlled_breaking_fill_rate_danger: Option<f32>,
    /// `dangerBreakingDecayRate`.
    pub(crate) danger_breaking_decay_rate: Option<f32>,
    /// `dangerBreakingFillRate`.
    pub(crate) danger_breaking_fill_rate: Option<f32>,
    /// `dangerBreakingFillRateExponent`.
    pub(crate) danger_breaking_fill_rate_exponent: Option<f32>,
    /// `dangerToGadgetDamage`.
    pub(crate) danger_to_gadget_damage: Option<f32>,
    /// `decayPerMass`.
    pub(crate) decay_per_mass: Option<f32>,
    /// `defaultMass`.
    pub(crate) default_mass: Option<f32>,
    /// `gadgetDestroyThreshold`.
    pub(crate) gadget_destroy_threshold: Option<f32>,
    /// `gadgetDetachThreshold`.
    pub(crate) gadget_detach_threshold: Option<f32>,
    /// `modifierPersistenceTime` in seconds.
    pub(crate) modifier_persistence_time: Option<f32>,
    /// `optimalWindowFactor`.
    pub(crate) optimal_window_factor: Option<f32>,
    /// `optimalWindowMaxSize`.
    pub(crate) optimal_window_max_size: Option<f32>,
    /// `optimalWindowSize`.
    pub(crate) optimal_window_size: Option<f32>,
    /// `optimalWindowThinnessCurveFactor`.
    pub(crate) optimal_window_thinness_curve_factor: Option<f32>,
    /// `powerCapacityPerMass`.
    pub(crate) power_capacity_per_mass: Option<f32>,
    /// `resistanceCurveFactor`.
    pub(crate) resistance_curve_factor: Option<f32>,
    /// `terrainFactorStaticThreshold`.
    pub(crate) terrain_factor_static_threshold: Option<f32>,
    /// `hitConsistencyParams`.
    pub(crate) hit_consistency: HitConsistency,
    /// `mineableInstabilityParams`.
    pub(crate) instability: InstabilityParams,
    /// `mineableExplosionParams`, without the default explosion.
    pub(crate) explosion: ExplosionPool,
    /// The GUID of the `ResourceType` Record the Reference `wasteResourceType` links to; `null`
    /// for a null Reference.
    pub(crate) waste_resource_type_guid: Option<CatalogGuid>,
}

/// `hitConsistencyParams` of a [`MiningGlobalParams`].
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct HitConsistency {
    /// `extractionMagnitude`.
    pub(crate) extraction_magnitude: Option<f32>,
    /// `hitHistoryWindow`.
    pub(crate) hit_history_window: Option<f32>,
    /// `maxEffectOnInstability`.
    pub(crate) max_effect_on_instability: Option<f32>,
    /// `minDeviation`.
    pub(crate) min_deviation: Option<f32>,
    /// `standardDeviationMultiplier`.
    pub(crate) standard_deviation_multiplier: Option<f32>,
    /// `timeExponent`.
    pub(crate) time_exponent: Option<f32>,
}

/// `mineableInstabilityParams` of a [`MiningGlobalParams`].
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct InstabilityParams {
    /// `instabilityCurveFactor`.
    pub(crate) curve_factor: Option<f32>,
    /// `instabilityWavePeriod`.
    pub(crate) wave_period: Option<f32>,
    /// `instabilityWaveVariance`.
    pub(crate) wave_variance: Option<f32>,
}

/// The numbers of `mineableExplosionParams` of a [`MiningGlobalParams`] besides the default
/// explosion.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ExplosionPool {
    /// `dangerPoolFactor`.
    pub(crate) danger_pool_factor: Option<f32>,
    /// `defaultVolume`.
    pub(crate) default_volume: Option<f32>,
}
