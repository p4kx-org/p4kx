// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::RecordTree;

use crate::contract_mining_params::{
    ExplosionPool, HitConsistency, InstabilityParams, MiningGlobalParams,
};
use crate::guid_text::CatalogGuid;
use crate::record_fields::{class_name, number, reference, structure};

/// Returns the mining parameters the Record tree `tree` describes.
pub(crate) fn mining_global_params<'f>(tree: &RecordTree<'f>) -> MiningGlobalParams<'f> {
    let root = Some(tree.root());
    let hit = structure(root, "hitConsistencyParams");
    let instability = structure(root, "mineableInstabilityParams");
    let explosion = structure(root, "mineableExplosionParams");
    MiningGlobalParams {
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        absorbable_volume_threshold: number(root, "absorbableVolumeThreshold"),
        c_scu_per_volume: number(root, "cSCUPerVolume"),
        child_rock_inactivity_lifetime: number(root, "childRockInactivityLifetime"),
        child_rock_invulnerability_time: number(root, "childRockInvulnerabilityTime"),
        child_rock_life_timer: number(root, "childRockLifeTimer"),
        child_rock_zero_g_damping: number(root, "childRockZeroGDamping"),
        composition_mass_multiplier: number(root, "compositionMassMultiplier"),
        controlled_breaking_decay_rate: number(root, "controlledBreakingDecayRate"),
        controlled_breaking_fill_rate: number(root, "controlledBreakingFillRate"),
        controlled_breaking_fill_rate_danger: number(root, "controlledBreakingFillRateDanger"),
        danger_breaking_decay_rate: number(root, "dangerBreakingDecayRate"),
        danger_breaking_fill_rate: number(root, "dangerBreakingFillRate"),
        danger_breaking_fill_rate_exponent: number(root, "dangerBreakingFillRateExponent"),
        danger_to_gadget_damage: number(root, "dangerToGadgetDamage"),
        decay_per_mass: number(root, "decayPerMass"),
        default_mass: number(root, "defaultMass"),
        gadget_destroy_threshold: number(root, "gadgetDestroyThreshold"),
        gadget_detach_threshold: number(root, "gadgetDetachThreshold"),
        modifier_persistence_time: number(root, "modifierPersistenceTime"),
        optimal_window_factor: number(root, "optimalWindowFactor"),
        optimal_window_max_size: number(root, "optimalWindowMaxSize"),
        optimal_window_size: number(root, "optimalWindowSize"),
        optimal_window_thinness_curve_factor: number(root, "optimalWindowThinnessCurveFactor"),
        power_capacity_per_mass: number(root, "powerCapacityPerMass"),
        resistance_curve_factor: number(root, "resistanceCurveFactor"),
        terrain_factor_static_threshold: number(root, "terrainFactorStaticThreshold"),
        hit_consistency: HitConsistency {
            extraction_magnitude: number(hit, "extractionMagnitude"),
            hit_history_window: number(hit, "hitHistoryWindow"),
            max_effect_on_instability: number(hit, "maxEffectOnInstability"),
            min_deviation: number(hit, "minDeviation"),
            standard_deviation_multiplier: number(hit, "standardDeviationMultiplier"),
            time_exponent: number(hit, "timeExponent"),
        },
        instability: InstabilityParams {
            curve_factor: number(instability, "instabilityCurveFactor"),
            wave_period: number(instability, "instabilityWavePeriod"),
            wave_variance: number(instability, "instabilityWaveVariance"),
        },
        explosion: ExplosionPool {
            danger_pool_factor: number(explosion, "dangerPoolFactor"),
            default_volume: number(explosion, "defaultVolume"),
        },
        waste_resource_type_guid: reference(root, "wasteResourceType"),
    }
}
