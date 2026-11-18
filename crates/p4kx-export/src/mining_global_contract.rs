// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

use super::blocks::{MiningExplosion, MiningHitConsistency, MiningInstability};

#[path = "mining_global_views.rs"]
mod views;

/// One physical mining configuration with exact values and its complete original field arena.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct MiningGlobalConfiguration<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    fields: SourceArenaOutput<'source>,
    absorbable_volume_threshold: SourcedValue<ResourceNumber>,
    #[serde(rename = "cSCUPerVolume")]
    c_scu_per_volume: SourcedValue<ResourceNumber>,
    child_rock_inactivity_lifetime: SourcedValue<ResourceNumber>,
    child_rock_invulnerability_time: SourcedValue<ResourceNumber>,
    child_rock_life_timer: SourcedValue<ResourceNumber>,
    child_rock_zero_g_damping: SourcedValue<ResourceNumber>,
    composition_mass_multiplier: SourcedValue<ResourceNumber>,
    controlled_breaking_decay_rate: SourcedValue<ResourceNumber>,
    controlled_breaking_fill_rate: SourcedValue<ResourceNumber>,
    controlled_breaking_fill_rate_danger: SourcedValue<ResourceNumber>,
    danger_breaking_decay_rate: SourcedValue<ResourceNumber>,
    danger_breaking_fill_rate: SourcedValue<ResourceNumber>,
    danger_breaking_fill_rate_exponent: SourcedValue<ResourceNumber>,
    danger_to_gadget_damage: SourcedValue<ResourceNumber>,
    decay_per_mass: SourcedValue<ResourceNumber>,
    default_mass: SourcedValue<ResourceNumber>,
    gadget_destroy_threshold: SourcedValue<ResourceNumber>,
    gadget_detach_threshold: SourcedValue<ResourceNumber>,
    modifier_persistence_time: SourcedValue<ResourceNumber>,
    optimal_window_factor: SourcedValue<ResourceNumber>,
    optimal_window_max_size: SourcedValue<ResourceNumber>,
    optimal_window_size: SourcedValue<ResourceNumber>,
    optimal_window_thinness_curve_factor: SourcedValue<ResourceNumber>,
    power_capacity_per_mass: SourcedValue<ResourceNumber>,
    resistance_curve_factor: SourcedValue<ResourceNumber>,
    terrain_factor_static_threshold: SourcedValue<ResourceNumber>,
    hit_consistency_params: SourcedValue<MiningHitConsistency>,
    mineable_instability_params: SourcedValue<MiningInstability>,
    mineable_explosion_params: SourcedValue<MiningExplosion>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    waste_resource_type: SourcedValue<Box<SourceRecordReference<'source>>>,
}

#[path = "mining_global_read.rs"]
mod read;
pub(super) use read::project;
