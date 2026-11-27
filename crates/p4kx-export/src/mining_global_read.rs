// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::super::blocks::{MiningExplosion, MiningHitConsistency, MiningInstability};
use super::MiningGlobalConfiguration;
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::authored_fields::Observed;
use crate::source_fields::FieldError;

pub(in crate::catalog_v3) fn project<'source>(
    source: CanonicalRecordHeader<'source>,
    fields: Observed<'_, '_, '_, 'source>,
    _projection: &CatalogProjection<'_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<MiningGlobalConfiguration<'source>, CatalogError> {
    Ok(MiningGlobalConfiguration {
        source,
        fields: fields.authored_members()?,
        absorbable_volume_threshold: fields.authored_number("absorbableVolumeThreshold", notes)?,
        c_scu_per_volume: fields.authored_number("cSCUPerVolume", notes)?,
        child_rock_inactivity_lifetime: fields
            .authored_number("childRockInactivityLifetime", notes)?,
        child_rock_invulnerability_time: fields
            .authored_number("childRockInvulnerabilityTime", notes)?,
        child_rock_life_timer: fields.authored_number("childRockLifeTimer", notes)?,
        child_rock_zero_g_damping: fields.authored_number("childRockZeroGDamping", notes)?,
        composition_mass_multiplier: fields.authored_number("compositionMassMultiplier", notes)?,
        controlled_breaking_decay_rate: fields
            .authored_number("controlledBreakingDecayRate", notes)?,
        controlled_breaking_fill_rate: fields
            .authored_number("controlledBreakingFillRate", notes)?,
        controlled_breaking_fill_rate_danger: fields
            .authored_number("controlledBreakingFillRateDanger", notes)?,
        danger_breaking_decay_rate: fields.authored_number("dangerBreakingDecayRate", notes)?,
        danger_breaking_fill_rate: fields.authored_number("dangerBreakingFillRate", notes)?,
        danger_breaking_fill_rate_exponent: fields
            .authored_number("dangerBreakingFillRateExponent", notes)?,
        danger_to_gadget_damage: fields.authored_number("dangerToGadgetDamage", notes)?,
        decay_per_mass: fields.authored_number("decayPerMass", notes)?,
        default_mass: fields.authored_number("defaultMass", notes)?,
        gadget_destroy_threshold: fields.authored_number("gadgetDestroyThreshold", notes)?,
        gadget_detach_threshold: fields.authored_number("gadgetDetachThreshold", notes)?,
        modifier_persistence_time: fields.authored_number("modifierPersistenceTime", notes)?,
        optimal_window_factor: fields.authored_number("optimalWindowFactor", notes)?,
        optimal_window_max_size: fields.authored_number("optimalWindowMaxSize", notes)?,
        optimal_window_size: fields.authored_number("optimalWindowSize", notes)?,
        optimal_window_thinness_curve_factor: fields
            .authored_number("optimalWindowThinnessCurveFactor", notes)?,
        power_capacity_per_mass: fields.authored_number("powerCapacityPerMass", notes)?,
        resistance_curve_factor: fields.authored_number("resistanceCurveFactor", notes)?,
        terrain_factor_static_threshold: fields
            .authored_number("terrainFactorStaticThreshold", notes)?,
        hit_consistency_params: MiningHitConsistency::read_source(fields, notes)?,
        mineable_instability_params: MiningInstability::read_source(fields, notes)?,
        mineable_explosion_params: MiningExplosion::read_source(fields, notes)?,
        waste_resource_type: fields.record_link("wasteResourceType", notes)?,
    })
}
