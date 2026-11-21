// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::SourceRecordReference;
use p4kx_export::catalog_queries::mining::{
    MiningExplosion, MiningGlobalConfiguration, MiningHitConsistency, MiningInstability,
    RefiningProcessDefinition,
};
use p4kx_extraction::SourceInventory;
use serde_json::Value;

use crate::canonical_header::{
    originals,
    wire::{TestResult, Wire},
};
use crate::catalog_resources::resource_consumer_walk::visit_header;

pub(super) fn visit_global(
    row: &MiningGlobalConfiguration<'_>,
    inventory: &SourceInventory,
    expected: &Value,
) -> TestResult {
    let wire = Wire::new(expected, inventory);
    visit_header(row.source(), wire)?;
    originals::visit_arena(row.fields(), wire.field("fields")?)?;
    visit_root_numbers(row, wire)?;
    wire.field("hitConsistencyParams")?
        .cell(row.hit_consistency_params(), visit_hit)?;
    wire.field("mineableInstabilityParams")?
        .cell(row.mineable_instability_params(), visit_instability)?;
    wire.field("mineableExplosionParams")?
        .cell(row.mineable_explosion_params(), visit_explosion)?;
    wire.field("wasteResourceType")?
        .cell(row.waste_resource_type(), |reference, wire| {
            visit_reference(reference, wire)
        })
}

fn visit_root_numbers(row: &MiningGlobalConfiguration<'_>, wire: Wire<'_>) -> TestResult {
    visit_root_numbers_first(row, wire)?;
    visit_root_numbers_second(row, wire)?;
    visit_root_numbers_third(row, wire)
}

fn visit_root_numbers_first(row: &MiningGlobalConfiguration<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("absorbableVolumeThreshold")?
        .cell(row.absorbable_volume_threshold(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("cSCUPerVolume")?
        .cell(row.c_scu_per_volume(), |number, wire| wire.number(number))?;
    wire.field("childRockInactivityLifetime")?
        .cell(row.child_rock_inactivity_lifetime(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("childRockInvulnerabilityTime")?
        .cell(row.child_rock_invulnerability_time(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("childRockLifeTimer")?
        .cell(row.child_rock_life_timer(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("childRockZeroGDamping")?
        .cell(row.child_rock_zero_g_damping(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("compositionMassMultiplier")?
        .cell(row.composition_mass_multiplier(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("controlledBreakingDecayRate")?
        .cell(row.controlled_breaking_decay_rate(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("controlledBreakingFillRate")?
        .cell(row.controlled_breaking_fill_rate(), |number, wire| {
            wire.number(number)
        })?;
    Ok(())
}

fn visit_root_numbers_second(row: &MiningGlobalConfiguration<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("controlledBreakingFillRateDanger")?.cell(
        row.controlled_breaking_fill_rate_danger(),
        |number, wire| wire.number(number),
    )?;
    wire.field("dangerBreakingDecayRate")?
        .cell(row.danger_breaking_decay_rate(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("dangerBreakingFillRate")?
        .cell(row.danger_breaking_fill_rate(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("dangerBreakingFillRateExponent")?
        .cell(row.danger_breaking_fill_rate_exponent(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("dangerToGadgetDamage")?
        .cell(row.danger_to_gadget_damage(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("decayPerMass")?
        .cell(row.decay_per_mass(), |number, wire| wire.number(number))?;
    wire.field("defaultMass")?
        .cell(row.default_mass(), |number, wire| wire.number(number))?;
    wire.field("gadgetDestroyThreshold")?
        .cell(row.gadget_destroy_threshold(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("gadgetDetachThreshold")?
        .cell(row.gadget_detach_threshold(), |number, wire| {
            wire.number(number)
        })?;
    Ok(())
}

fn visit_root_numbers_third(row: &MiningGlobalConfiguration<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("modifierPersistenceTime")?
        .cell(row.modifier_persistence_time(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("optimalWindowFactor")?
        .cell(row.optimal_window_factor(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("optimalWindowMaxSize")?
        .cell(row.optimal_window_max_size(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("optimalWindowSize")?
        .cell(row.optimal_window_size(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("optimalWindowThinnessCurveFactor")?.cell(
        row.optimal_window_thinness_curve_factor(),
        |number, wire| wire.number(number),
    )?;
    wire.field("powerCapacityPerMass")?
        .cell(row.power_capacity_per_mass(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("resistanceCurveFactor")?
        .cell(row.resistance_curve_factor(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("terrainFactorStaticThreshold")?
        .cell(row.terrain_factor_static_threshold(), |number, wire| {
            wire.number(number)
        })?;
    Ok(())
}

fn visit_hit(row: &MiningHitConsistency, wire: Wire<'_>) -> TestResult {
    wire.field("extractionMagnitude")?
        .cell(row.extraction_magnitude(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("hitHistoryWindow")?
        .cell(row.hit_history_window(), |number, wire| wire.number(number))?;
    wire.field("maxEffectOnInstability")?
        .cell(row.max_effect_on_instability(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("minDeviation")?
        .cell(row.min_deviation(), |number, wire| wire.number(number))?;
    wire.field("standardDeviationMultiplier")?
        .cell(row.standard_deviation_multiplier(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("timeExponent")?
        .cell(row.time_exponent(), |number, wire| wire.number(number))?;
    Ok(())
}

fn visit_instability(row: &MiningInstability, wire: Wire<'_>) -> TestResult {
    wire.field("instabilityCurveFactor")?
        .cell(row.instability_curve_factor(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("instabilityWavePeriod")?
        .cell(row.instability_wave_period(), |number, wire| {
            wire.number(number)
        })?;
    wire.field("instabilityWaveVariance")?
        .cell(row.instability_wave_variance(), |number, wire| {
            wire.number(number)
        })?;
    Ok(())
}

fn visit_explosion(row: &MiningExplosion, wire: Wire<'_>) -> TestResult {
    wire.field("dangerPoolFactor")?
        .cell(row.danger_pool_factor(), |number, wire| wire.number(number))?;
    wire.field("defaultVolume")?
        .cell(row.default_volume(), |number, wire| wire.number(number))?;
    Ok(())
}

fn visit_reference(reference: &SourceRecordReference<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("guid")?.guid(&reference.guid())?;
    wire.field("targetRecordIndex")?
        .leaf(reference.target_record_index())?;
    wire.field("recordPath")?.leaf(reference.record_path())?;
    wire.field("targetType")?.leaf(reference.target_type())?;
    wire.field("status")?.raw(reference.status())
}

pub(super) fn visit_refining(
    row: &RefiningProcessDefinition<'_>,
    inventory: &SourceInventory,
    expected: &Value,
) -> TestResult {
    let wire = Wire::new(expected, inventory);
    visit_header(row.source(), wire)?;
    originals::visit_arena(row.fields(), wire.field("fields")?)?;
    wire.field("processName")?.leaf(row.process_name())?;
    wire.field("refiningSpeed")?.leaf(row.refining_speed())?;
    wire.field("refiningQuality")?.leaf(row.refining_quality())
}
