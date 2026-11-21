// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;

use super::super::blocks::{MiningExplosion, MiningHitConsistency, MiningInstability};
use super::MiningGlobalConfiguration;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::resource_number::ResourceNumber;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

impl<'source> MiningGlobalConfiguration<'source> {
    /// Borrows the independently sourced physical Record header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }
    /// Borrows every original field and occurrence of this physical owner.
    #[must_use]
    pub const fn fields(&self) -> &SourceArenaOutput<'source> {
        &self.fields
    }
    /// Borrows the exact original `absorbableVolumeThreshold` value.
    #[must_use]
    pub const fn absorbable_volume_threshold(&self) -> &SourcedValue<ResourceNumber> {
        &self.absorbable_volume_threshold
    }
    /// Borrows the exact original `cSCUPerVolume` value.
    #[must_use]
    pub const fn c_scu_per_volume(&self) -> &SourcedValue<ResourceNumber> {
        &self.c_scu_per_volume
    }
    /// Borrows the exact original `childRockInactivityLifetime` value.
    #[must_use]
    pub const fn child_rock_inactivity_lifetime(&self) -> &SourcedValue<ResourceNumber> {
        &self.child_rock_inactivity_lifetime
    }
    /// Borrows the exact original `childRockInvulnerabilityTime` value.
    #[must_use]
    pub const fn child_rock_invulnerability_time(&self) -> &SourcedValue<ResourceNumber> {
        &self.child_rock_invulnerability_time
    }
    /// Borrows the exact original `childRockLifeTimer` value.
    #[must_use]
    pub const fn child_rock_life_timer(&self) -> &SourcedValue<ResourceNumber> {
        &self.child_rock_life_timer
    }
    /// Borrows the exact original `childRockZeroGDamping` value.
    #[must_use]
    pub const fn child_rock_zero_g_damping(&self) -> &SourcedValue<ResourceNumber> {
        &self.child_rock_zero_g_damping
    }
    /// Borrows the exact original `compositionMassMultiplier` value.
    #[must_use]
    pub const fn composition_mass_multiplier(&self) -> &SourcedValue<ResourceNumber> {
        &self.composition_mass_multiplier
    }
    /// Borrows the exact original `controlledBreakingDecayRate` value.
    #[must_use]
    pub const fn controlled_breaking_decay_rate(&self) -> &SourcedValue<ResourceNumber> {
        &self.controlled_breaking_decay_rate
    }
    /// Borrows the exact original `controlledBreakingFillRate` value.
    #[must_use]
    pub const fn controlled_breaking_fill_rate(&self) -> &SourcedValue<ResourceNumber> {
        &self.controlled_breaking_fill_rate
    }
    /// Borrows the exact original `controlledBreakingFillRateDanger` value.
    #[must_use]
    pub const fn controlled_breaking_fill_rate_danger(&self) -> &SourcedValue<ResourceNumber> {
        &self.controlled_breaking_fill_rate_danger
    }
    /// Borrows the exact original `dangerBreakingDecayRate` value.
    #[must_use]
    pub const fn danger_breaking_decay_rate(&self) -> &SourcedValue<ResourceNumber> {
        &self.danger_breaking_decay_rate
    }
    /// Borrows the exact original `dangerBreakingFillRate` value.
    #[must_use]
    pub const fn danger_breaking_fill_rate(&self) -> &SourcedValue<ResourceNumber> {
        &self.danger_breaking_fill_rate
    }
    /// Borrows the exact original `dangerBreakingFillRateExponent` value.
    #[must_use]
    pub const fn danger_breaking_fill_rate_exponent(&self) -> &SourcedValue<ResourceNumber> {
        &self.danger_breaking_fill_rate_exponent
    }
    /// Borrows the exact original `dangerToGadgetDamage` value.
    #[must_use]
    pub const fn danger_to_gadget_damage(&self) -> &SourcedValue<ResourceNumber> {
        &self.danger_to_gadget_damage
    }
    /// Borrows the exact original `decayPerMass` value.
    #[must_use]
    pub const fn decay_per_mass(&self) -> &SourcedValue<ResourceNumber> {
        &self.decay_per_mass
    }
    /// Borrows the exact original `defaultMass` value.
    #[must_use]
    pub const fn default_mass(&self) -> &SourcedValue<ResourceNumber> {
        &self.default_mass
    }
    /// Borrows the exact original `gadgetDestroyThreshold` value.
    #[must_use]
    pub const fn gadget_destroy_threshold(&self) -> &SourcedValue<ResourceNumber> {
        &self.gadget_destroy_threshold
    }
    /// Borrows the exact original `gadgetDetachThreshold` value.
    #[must_use]
    pub const fn gadget_detach_threshold(&self) -> &SourcedValue<ResourceNumber> {
        &self.gadget_detach_threshold
    }
    /// Borrows the exact original `modifierPersistenceTime` value.
    #[must_use]
    pub const fn modifier_persistence_time(&self) -> &SourcedValue<ResourceNumber> {
        &self.modifier_persistence_time
    }
    /// Borrows the exact original `optimalWindowFactor` value.
    #[must_use]
    pub const fn optimal_window_factor(&self) -> &SourcedValue<ResourceNumber> {
        &self.optimal_window_factor
    }
    /// Borrows the exact original `optimalWindowMaxSize` value.
    #[must_use]
    pub const fn optimal_window_max_size(&self) -> &SourcedValue<ResourceNumber> {
        &self.optimal_window_max_size
    }
    /// Borrows the exact original `optimalWindowSize` value.
    #[must_use]
    pub const fn optimal_window_size(&self) -> &SourcedValue<ResourceNumber> {
        &self.optimal_window_size
    }
    /// Borrows the exact original `optimalWindowThinnessCurveFactor` value.
    #[must_use]
    pub const fn optimal_window_thinness_curve_factor(&self) -> &SourcedValue<ResourceNumber> {
        &self.optimal_window_thinness_curve_factor
    }
    /// Borrows the exact original `powerCapacityPerMass` value.
    #[must_use]
    pub const fn power_capacity_per_mass(&self) -> &SourcedValue<ResourceNumber> {
        &self.power_capacity_per_mass
    }
    /// Borrows the exact original `resistanceCurveFactor` value.
    #[must_use]
    pub const fn resistance_curve_factor(&self) -> &SourcedValue<ResourceNumber> {
        &self.resistance_curve_factor
    }
    /// Borrows the exact original `terrainFactorStaticThreshold` value.
    #[must_use]
    pub const fn terrain_factor_static_threshold(&self) -> &SourcedValue<ResourceNumber> {
        &self.terrain_factor_static_threshold
    }
    /// Borrows the state and numeric fields of `hitConsistencyParams`.
    #[must_use]
    pub const fn hit_consistency_params(&self) -> &SourcedValue<MiningHitConsistency> {
        &self.hit_consistency_params
    }
    /// Borrows the state and numeric fields of `mineableInstabilityParams`.
    #[must_use]
    pub const fn mineable_instability_params(&self) -> &SourcedValue<MiningInstability> {
        &self.mineable_instability_params
    }
    /// Borrows the state and numeric fields of `mineableExplosionParams`.
    #[must_use]
    pub const fn mineable_explosion_params(&self) -> &SourcedValue<MiningExplosion> {
        &self.mineable_explosion_params
    }
    /// Borrows the original waste reference without reading its target body.
    #[must_use]
    pub const fn waste_resource_type(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.waste_resource_type
    }
}
