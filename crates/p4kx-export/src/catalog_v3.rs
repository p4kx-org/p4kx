// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "catalog_number_capture.rs"]
pub(crate) mod catalog_number_capture;
#[path = "catalog_number_diagnostics.rs"]
pub(crate) mod catalog_number_diagnostics;
#[path = "catalog_number_health.rs"]
pub(crate) mod catalog_number_health;
#[path = "number_position.rs"]
pub(crate) mod number_position;

#[path = "blueprint_category_list.rs"]
pub(crate) mod blueprint_category_list;
#[path = "blueprint_category_list_read.rs"]
pub(crate) mod blueprint_category_list_read;

#[path = "durability_damage.rs"]
mod authored_damage;
#[path = "authored_explosion.rs"]
mod authored_explosion;
#[path = "durability_observed.rs"]
mod authored_fields;
#[path = "durability_link.rs"]
mod authored_link;
#[path = "authored_nodes.rs"]
mod authored_nodes;

#[path = "explosion_definition.rs"]
pub(crate) mod explosion_definition;

#[path = "ammo_definition.rs"]
pub(crate) mod ammo_definition;
#[path = "ammo_presentation.rs"]
mod ammo_presentation;
#[path = "authored_ammo_inputs.rs"]
mod authored_ammo_inputs;
#[path = "authored_effect_resource.rs"]
mod authored_effect_resource;
#[path = "authored_projectile.rs"]
mod authored_projectile;
#[path = "authored_rgb.rs"]
mod authored_rgb;
#[path = "catalog_ammo_bindings.rs"]
pub(crate) mod catalog_ammo_bindings;
#[path = "catalog_ammo_containers.rs"]
pub(crate) mod catalog_ammo_containers;
#[path = "catalog_declarations.rs"]
pub(crate) mod catalog_declarations;
#[path = "catalog_explosives.rs"]
pub(crate) mod catalog_explosives;
#[path = "catalog_hacking_chips.rs"]
pub(crate) mod catalog_hacking_chips;
#[path = "catalog_item_alignment.rs"]
pub(crate) mod catalog_item_alignment;
#[path = "catalog_item_components.rs"]
pub(crate) mod catalog_item_components;
#[path = "catalog_quantum_drive.rs"]
pub(crate) mod catalog_quantum_drive;
#[path = "catalog_shield_generator.rs"]
pub(crate) mod catalog_shield_generator;
#[path = "catalog_weapon_actions.rs"]
pub(crate) mod catalog_weapon_actions;
#[path = "catalog_weapon_magazine_slots.rs"]
mod catalog_weapon_magazine_slots;
#[path = "item_alignment_options.rs"]
mod item_alignment_options;
#[path = "item_component_owner.rs"]
mod item_component_owner;
#[path = "ordnance_fields.rs"]
mod ordnance_fields;
#[path = "projectile_range.rs"]
mod projectile_range;
#[path = "quantum_boost.rs"]
mod quantum_boost;
#[path = "quantum_fields.rs"]
mod quantum_fields;
#[path = "quantum_heat.rs"]
mod quantum_heat;
#[path = "quantum_profile.rs"]
mod quantum_profile;
#[path = "sensor_mine.rs"]
mod sensor_mine;

#[path = "catalog_durability.rs"]
pub(crate) mod catalog_durability;
#[path = "tag_catalog.rs"]
pub(crate) mod tag_catalog;

#[path = "catalog_definitions.rs"]
pub(crate) mod catalog_definitions;
#[path = "catalog_origins.rs"]
pub(crate) mod catalog_origins;
#[path = "catalog_projection.rs"]
pub(crate) mod catalog_projection;
#[path = "catalog_read.rs"]
pub(crate) mod catalog_read;
#[path = "catalog_record_reference.rs"]
pub(crate) mod catalog_record_reference;
#[path = "catalog_text_contract.rs"]
pub(crate) mod catalog_text_contract;
#[path = "catalog_text_forms.rs"]
pub(crate) mod catalog_text_forms;
#[path = "gameplay_property_catalog.rs"]
pub(crate) mod gameplay_property_catalog;
#[path = "loot_filter_catalog.rs"]
pub(crate) mod loot_filter_catalog;
#[path = "quality_bands_read.rs"]
pub(crate) mod quality_bands_read;
#[path = "quality_curve_source_read.rs"]
pub(crate) mod quality_curve_source_read;
#[path = "quality_source.rs"]
pub(crate) mod quality_source;
#[path = "quality_source_read.rs"]
pub(crate) mod quality_source_read;
#[path = "resource_density_read.rs"]
pub(crate) mod resource_density_read;
#[path = "resource_density_source.rs"]
pub(crate) mod resource_density_source;
#[path = "resource_float.rs"]
pub(crate) mod resource_float;
#[path = "resource_group.rs"]
pub(crate) mod resource_group;
#[path = "resource_group_catalog.rs"]
pub(crate) mod resource_group_catalog;
#[path = "resource_group_read.rs"]
pub(crate) mod resource_group_read;
#[path = "resource_integer.rs"]
pub(crate) mod resource_integer;
#[path = "resource_memberships.rs"]
pub(crate) mod resource_memberships;
#[path = "resource_number.rs"]
pub(crate) mod resource_number;
#[path = "resource_property_contract.rs"]
pub(crate) mod resource_property_contract;
#[path = "resource_property_read.rs"]
pub(crate) mod resource_property_read;
#[path = "resource_raw_policy.rs"]
pub(crate) mod resource_raw_policy;
#[path = "resource_read.rs"]
pub(crate) mod resource_read;
#[path = "resource_type_contract.rs"]
pub(crate) mod resource_type_contract;
#[path = "resource_value_read.rs"]
pub(crate) mod resource_value_read;
#[path = "source_value_catalog.rs"]
pub(crate) mod source_value_catalog;

#[path = "catalog_projections.rs"]
pub(crate) mod catalog_projections;

#[path = "catalog_table.rs"]
pub(crate) mod catalog_table;

#[path = "commodity.rs"]
pub(crate) mod commodity;

#[path = "commodity_authoring_contract.rs"]
pub(crate) mod commodity_authoring_contract;

#[path = "commodity_authoring_inputs.rs"]
pub(crate) mod commodity_authoring_inputs;

#[path = "commodity_authoring_read.rs"]
pub(crate) mod commodity_authoring_read;

#[path = "commodity_backfill.rs"]
pub(crate) mod commodity_backfill;

#[path = "commodity_binding_read.rs"]
pub(crate) mod commodity_binding_read;

#[path = "commodity_composition_read.rs"]
pub(crate) mod commodity_composition_read;

#[path = "commodity_contract.rs"]
pub(crate) mod commodity_contract;

#[path = "commodity_inputs.rs"]
pub(crate) mod commodity_inputs;

#[path = "commodity_presentation.rs"]
pub(crate) mod commodity_presentation;

#[path = "commodity_resource.rs"]
pub(crate) mod commodity_resource;

#[path = "commodity_resource_presentation.rs"]
pub(crate) mod commodity_resource_presentation;

#[path = "commodity_source.rs"]
pub(crate) mod commodity_source;

#[path = "commodity_text_choice.rs"]
pub(crate) mod commodity_text_choice;

#[path = "commodity_text_input.rs"]
pub(crate) mod commodity_text_input;

#[path = "canonical_record_header.rs"]
pub(crate) mod canonical_record_header;
#[path = "record_header_field.rs"]
pub(crate) mod record_header_field;
#[path = "source_header.rs"]
pub(crate) mod source_header;

#[path = "export_health_capture.rs"]
pub(crate) mod export_health_capture;
