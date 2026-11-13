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

#[path = "faction_definition.rs"]
pub(crate) mod faction_definition;
#[path = "reputation_faction_definition.rs"]
pub(crate) mod reputation_faction_definition;
#[path = "reputation_perk.rs"]
mod reputation_perk;
#[path = "reputation_perk_list.rs"]
pub(crate) mod reputation_perk_list;
#[path = "reputation_relation.rs"]
mod reputation_relation;
#[path = "reputation_reward_amount_definition.rs"]
pub(crate) mod reputation_reward_amount_definition;
#[path = "reputation_trigger_configuration.rs"]
pub(crate) mod reputation_trigger_configuration;
#[path = "sandbox_trigger.rs"]
mod sandbox_trigger;
#[path = "sandbox_trigger_record_definition.rs"]
pub(crate) mod sandbox_trigger_record_definition;

#[path = "auto_spawn_class.rs"]
mod auto_spawn_class;
#[path = "auto_spawn_data.rs"]
mod auto_spawn_data;
#[path = "auto_spawn_definition.rs"]
mod auto_spawn_definition;
#[path = "catalog_placements.rs"]
pub(crate) mod catalog_placements;
#[path = "explosion_definition.rs"]
pub(crate) mod explosion_definition;
#[path = "facility_instance.rs"]
mod facility_instance;
#[path = "facility_loadout_placements.rs"]
mod facility_loadout_placements;
#[path = "infraction_definition.rs"]
pub(crate) mod infraction_definition;
#[path = "infraction_parameters.rs"]
mod infraction_parameters;
#[path = "jurisdiction_definition.rs"]
pub(crate) mod jurisdiction_definition;
#[path = "law_kinds.rs"]
pub(crate) mod law_kinds;
#[path = "loadout_position.rs"]
mod loadout_position;
#[path = "location_classification.rs"]
pub(crate) mod location_classification;
#[path = "location_vocabulary.rs"]
pub(crate) mod location_vocabulary;
#[path = "location_vocabulary_definition.rs"]
mod location_vocabulary_definition;
#[path = "mission_scenario_definition.rs"]
pub(crate) mod mission_scenario_definition;
#[path = "npc_spawn_definition.rs"]
mod npc_spawn_definition;
#[path = "npc_spawn_definitions.rs"]
pub(crate) mod npc_spawn_definitions;
#[path = "prepared_definition.rs"]
mod prepared_definition;
#[path = "reputation_blackboard.rs"]
mod reputation_blackboard;
#[path = "reputation_context_definition.rs"]
pub(crate) mod reputation_context_definition;
#[path = "reputation_scope_definition.rs"]
pub(crate) mod reputation_scope_definition;
#[path = "reputation_standing_definition.rs"]
pub(crate) mod reputation_standing_definition;
#[path = "starmap_amenity_definition.rs"]
pub(crate) mod starmap_amenity_definition;
#[path = "starmap_definition_kinds.rs"]
pub(crate) mod starmap_definition_kinds;
#[path = "starmap_display.rs"]
mod starmap_display;
#[path = "starmap_navigation.rs"]
mod starmap_navigation;
#[path = "starmap_object_definition.rs"]
pub(crate) mod starmap_object_definition;
#[path = "starmap_object_links.rs"]
mod starmap_object_links;
#[path = "starmap_object_owned.rs"]
mod starmap_object_owned;
#[path = "starmap_placement.rs"]
mod starmap_placement;
#[path = "starmap_preset_definition.rs"]
pub(crate) mod starmap_preset_definition;
#[path = "starmap_radar.rs"]
mod starmap_radar;
#[path = "starmap_type_definition.rs"]
pub(crate) mod starmap_type_definition;
#[path = "starmap_visibility.rs"]
mod starmap_visibility;

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
#[path = "cargo_source.rs"]
pub(crate) mod cargo_source;
#[path = "catalog_ammo_bindings.rs"]
pub(crate) mod catalog_ammo_bindings;
#[path = "catalog_ammo_containers.rs"]
pub(crate) mod catalog_ammo_containers;
#[path = "catalog_capacity.rs"]
mod catalog_capacity;
#[path = "catalog_declarations.rs"]
pub(crate) mod catalog_declarations;
#[path = "catalog_entity_configurations.rs"]
pub(crate) mod catalog_entity_configurations;
#[path = "catalog_explosives.rs"]
pub(crate) mod catalog_explosives;
#[path = "catalog_facility.rs"]
mod catalog_facility;
#[path = "catalog_fuel.rs"]
mod catalog_fuel;
#[path = "catalog_fuel_configurations.rs"]
mod catalog_fuel_configurations;
#[path = "catalog_hacking_chips.rs"]
pub(crate) mod catalog_hacking_chips;
#[path = "catalog_independent_configurations.rs"]
pub(crate) mod catalog_independent_configurations;
#[path = "catalog_interactions.rs"]
mod catalog_interactions;
#[path = "catalog_item_alignment.rs"]
pub(crate) mod catalog_item_alignment;
#[path = "catalog_item_components.rs"]
pub(crate) mod catalog_item_components;
#[path = "catalog_mission_board.rs"]
mod catalog_mission_board;
#[path = "catalog_mission_board_read.rs"]
mod catalog_mission_board_read;
#[path = "catalog_quantum_drive.rs"]
pub(crate) mod catalog_quantum_drive;
#[path = "catalog_resource_component.rs"]
mod catalog_resource_component;
#[path = "catalog_resource_connection.rs"]
mod catalog_resource_connection;
#[path = "catalog_resource_container.rs"]
mod catalog_resource_container;
#[path = "catalog_resource_control_parameter.rs"]
mod catalog_resource_control_parameter;
#[path = "catalog_resource_delta.rs"]
mod catalog_resource_delta;
#[path = "catalog_resource_delta_amount.rs"]
mod catalog_resource_delta_amount;
#[path = "catalog_resource_flow_unit.rs"]
mod catalog_resource_flow_unit;
#[path = "catalog_resource_heat_connection.rs"]
mod catalog_resource_heat_connection;
#[path = "catalog_resource_network_configuration.rs"]
pub(crate) mod catalog_resource_network_configuration;
#[path = "catalog_resource_network_read.rs"]
pub(crate) mod catalog_resource_network_read;
#[path = "catalog_resource_power_connection.rs"]
mod catalog_resource_power_connection;
#[path = "catalog_resource_power_ranges.rs"]
mod catalog_resource_power_ranges;
#[path = "catalog_resource_signature.rs"]
mod catalog_resource_signature;
#[path = "catalog_resource_state.rs"]
mod catalog_resource_state;
#[path = "catalog_seat.rs"]
mod catalog_seat;
#[path = "catalog_seat_ejection.rs"]
mod catalog_seat_ejection;
#[path = "catalog_shield_generator.rs"]
pub(crate) mod catalog_shield_generator;
#[path = "catalog_unit_source.rs"]
mod catalog_unit_source;
#[path = "catalog_weapon_actions.rs"]
pub(crate) mod catalog_weapon_actions;
#[path = "catalog_weapon_magazine_slots.rs"]
mod catalog_weapon_magazine_slots;
#[path = "interactable_configuration.rs"]
mod interactable_configuration;
#[path = "interaction_conditions.rs"]
mod interaction_conditions;
#[path = "interaction_point.rs"]
mod interaction_point;
#[path = "interaction_source.rs"]
mod interaction_source;
#[path = "interaction_state.rs"]
mod interaction_state;
#[path = "interaction_state_edges.rs"]
mod interaction_state_edges;
#[path = "interaction_state_type.rs"]
mod interaction_state_type;
#[path = "interaction_task.rs"]
mod interaction_task;
#[path = "inventory_container_definition.rs"]
pub(crate) mod inventory_container_definition;
#[path = "inventory_container_type.rs"]
mod inventory_container_type;
#[path = "inventory_dimensions.rs"]
mod inventory_dimensions;
#[path = "inventory_entity_configurations.rs"]
mod inventory_entity_configurations;
#[path = "inventory_grid_cell.rs"]
mod inventory_grid_cell;
#[path = "inventory_item_occupancy.rs"]
mod inventory_item_occupancy;
#[path = "inventory_item_type_filter.rs"]
mod inventory_item_type_filter;
#[path = "inventory_open_container.rs"]
mod inventory_open_container;
#[path = "inventory_owner_bounds.rs"]
mod inventory_owner_bounds;
#[path = "inventory_owner_storage.rs"]
mod inventory_owner_storage;
#[path = "item_alignment_options.rs"]
mod item_alignment_options;
#[path = "item_component_owner.rs"]
mod item_component_owner;
#[path = "medical/mod.rs"]
pub(crate) mod medical;
#[path = "mission_sources.rs"]
pub(crate) mod mission_sources;
#[path = "network_endpoint.rs"]
mod network_endpoint;
#[path = "ordnance_fields.rs"]
mod ordnance_fields;
#[path = "port_restrictions.rs"]
mod port_restrictions;
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
#[path = "resource_network_links.rs"]
mod resource_network_links;
#[path = "resource_network_placements.rs"]
pub(crate) mod resource_network_placements;
#[path = "resource_topology_declarations.rs"]
mod resource_topology_declarations;
#[path = "resource_topology_index.rs"]
mod resource_topology_index;
#[path = "resource_topology_projection.rs"]
pub(crate) mod resource_topology_projection;
#[path = "room_associations.rs"]
mod room_associations;
#[path = "sensor_mine.rs"]
mod sensor_mine;
#[path = "shared_interaction.rs"]
mod shared_interaction;

#[path = "catalog_durability.rs"]
pub(crate) mod catalog_durability;
#[path = "catalog_vehicle_armor.rs"]
mod catalog_vehicle_armor;
#[path = "degradation_curve.rs"]
mod degradation_curve;
#[path = "tag_catalog.rs"]
pub(crate) mod tag_catalog;
#[path = "vehicle_armor_damage.rs"]
mod vehicle_armor_damage;
#[path = "vehicle_armor_resistance.rs"]
mod vehicle_armor_resistance;

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
