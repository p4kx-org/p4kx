// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::lifecycle::Lifecycle;
use super::observed::{Observed, ReadHealth};
use super::resistance::DurabilityResistances;
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// Original Health, repair, salvage and destruction fields of one component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct HealthFields<'source> {
    health: SourcedValue<ResourceFloat32>,
    damage_cap: SourcedValue<ResourceFloat32>,
    client_only: SourcedValue<bool>,
    unlock_interactions_on_death: SourcedValue<bool>,
    push_damage_up_parts_hierarchy_when_dead: SourcedValue<bool>,
    detach_from_item_port_on_death: SourcedValue<bool>,
    detach_from_entity_on_death: SourcedValue<bool>,
    destroy_self_on_death: SourcedValue<bool>,
    destroy_children_on_death: SourcedValue<bool>,
    propagate_explosion_damage_to_children: SourcedValue<bool>,
    use_dirt_shader_for_damage: SourcedValue<bool>,
    update_observable_status: SourcedValue<bool>,
    explosion_delay_time: SourcedValue<ResourceFloat32>,
    explosion_random_delay_range: SourcedValue<ResourceFloat32>,
    salvageable: SourcedValue<bool>,
    repairable: SourcedValue<bool>,
    salvage_damage_modifier: SourcedValue<ResourceFloat32>,
    mirror_salvage_hit: SourcedValue<bool>,
    disable_material_damage: SourcedValue<bool>,
    always_propagate_hit_to_parent: SourcedValue<bool>,
    duplicate_hit_to_parent: SourcedValue<bool>,
    send_on_destruction_event_to_master_root_container: SourcedValue<bool>,
    apply_explosion_falloff_from_origin: SourcedValue<bool>,
    serialized_damage_map_path: SourcedValue<String>,
    destroyed_geometry_tag: SourcedValue<String>,
    exploded_geometry_tag: SourcedValue<String>,
    debris_geometry_tag: SourcedValue<String>,
    explosion_bone: SourcedValue<String>,
    damage_resistances: SourcedValue<DurabilityResistances<'source>>,
    #[serde(flatten)]
    lifecycle: Lifecycle<'source>,
}

impl<'source> HealthFields<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<(Self, Option<f32>), CatalogError> {
        let ReadHealth {
            value,
            legacy_value,
        } = source.health(notes)?;
        Ok((
            Self {
                health: value,
                damage_cap: source.single("DamageCap", notes)?,
                client_only: source.boolean("ClientOnly", notes)?,
                unlock_interactions_on_death: source.boolean("UnlockInteractionsOnDeath", notes)?,
                push_damage_up_parts_hierarchy_when_dead: source
                    .boolean("PushDamageUpPartsHeirarchyWhenDead", notes)?,
                detach_from_item_port_on_death: source
                    .boolean("DetachFromItemPortOnDeath", notes)?,
                detach_from_entity_on_death: source.boolean("DetachFromEntityOnDeath", notes)?,
                destroy_self_on_death: source.boolean("DestroySelfOnDeath", notes)?,
                destroy_children_on_death: source.boolean("DestroyChildrenOnDeath", notes)?,
                propagate_explosion_damage_to_children: source
                    .boolean("PropagateExplosionDamageToChildren", notes)?,
                use_dirt_shader_for_damage: source.boolean("UseDirtShaderForDamage", notes)?,
                update_observable_status: source.boolean("UpdateObservableStatus", notes)?,
                explosion_delay_time: source.single("ExplosionDelayTime", notes)?,
                explosion_random_delay_range: source.single("ExplosionRandomDelayRange", notes)?,
                salvageable: source.boolean("IsSalvagable", notes)?,
                repairable: source.boolean("IsRepairable", notes)?,
                salvage_damage_modifier: source.single("SalvageDamageModifier", notes)?,
                mirror_salvage_hit: source.boolean("MirrorSalvageHit", notes)?,
                disable_material_damage: source.boolean("DisableMaterialDamage", notes)?,
                always_propagate_hit_to_parent: source
                    .boolean("alwaysPropagateHitToParent", notes)?,
                duplicate_hit_to_parent: source.boolean("duplicateHitToParent", notes)?,
                send_on_destruction_event_to_master_root_container: source
                    .boolean("sendOnDestructionEventToMasterRootContainer", notes)?,
                apply_explosion_falloff_from_origin: source
                    .boolean("applyExplosionFalloffFromOrigin", notes)?,
                serialized_damage_map_path: source.string("SerializedDamageMapPath", notes)?,
                destroyed_geometry_tag: source.string("DestroyedGeometryTag", notes)?,
                exploded_geometry_tag: source.string("ExplodedGeometryTag", notes)?,
                debris_geometry_tag: source.string("DebrisGeometryTag", notes)?,
                explosion_bone: source.string("ExplosionBone", notes)?,
                damage_resistances: source.owned(
                    "DamageResistances",
                    DurabilityResistances::TYPES,
                    notes,
                    DurabilityResistances::read,
                )?,
                lifecycle: Lifecycle::read(source, notes)?,
            },
            legacy_value,
        ))
    }
}
