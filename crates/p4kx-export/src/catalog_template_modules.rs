// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Canonical mission source and its derived configuration modules.

#[path = "catalog_configurations.rs"]
pub(crate) mod catalog_configurations;

#[path = "location_condition_kind.rs"]
pub(crate) mod location_condition_kind;

#[path = "scoped_location_kind.rs"]
pub(crate) mod scoped_location_kind;

#[path = "scoped_location_shape.rs"]
pub(crate) mod scoped_location_shape;

#[path = "spawn_tag_text.rs"]
pub(crate) mod spawn_tag_text;

#[path = "template_action_meaning.rs"]
pub(crate) mod template_action_meaning;

#[path = "template_action_read.rs"]
pub(crate) mod template_action_read;

#[path = "template_binding_read.rs"]
pub(crate) mod template_binding_read;

#[path = "template_canonical_read.rs"]
pub(crate) mod template_canonical_read;

#[path = "template_condition_read.rs"]
pub(crate) mod template_condition_read;

#[path = "template_configuration.rs"]
pub(crate) mod template_configuration;

#[path = "template_context_error.rs"]
pub(crate) mod template_context_error;

#[path = "template_context_owner_read.rs"]
pub(crate) mod template_context_owner_read;

#[path = "template_context_read.rs"]
pub(crate) mod template_context_read;

#[path = "template_context_source_read.rs"]
pub(crate) mod template_context_source_read;

#[path = "template_context_tag_read.rs"]
pub(crate) mod template_context_tag_read;

#[path = "template_context_value.rs"]
pub(crate) mod template_context_value;

#[path = "template_context_weak_read.rs"]
pub(crate) mod template_context_weak_read;

#[path = "template_declaration_binding.rs"]
pub(crate) mod template_declaration_binding;

#[path = "template_definition.rs"]
pub(crate) mod template_definition;

#[path = "template_dependency_spawns.rs"]
pub(crate) mod template_dependency_spawns;

#[path = "template_derived_failures.rs"]
pub(crate) mod template_derived_failures;

#[path = "template_field_binding.rs"]
pub(crate) mod template_field_binding;

#[path = "template_field_capture.rs"]
pub(crate) mod template_field_capture;

#[path = "template_flow_configuration_read.rs"]
pub(crate) mod template_flow_configuration_read;

#[path = "template_flow_plan.rs"]
pub(crate) mod template_flow_plan;

#[path = "template_hauling_projection.rs"]
pub(crate) mod template_hauling_projection;

#[path = "template_hauling_read.rs"]
pub(crate) mod template_hauling_read;

#[path = "template_input.rs"]
pub(crate) mod template_input;

#[path = "template_input_read.rs"]
pub(crate) mod template_input_read;

#[path = "template_localization_plan.rs"]
pub(crate) mod template_localization_plan;

#[path = "template_localization_read.rs"]
pub(crate) mod template_localization_read;

#[path = "template_meaning_fields.rs"]
pub(crate) mod template_meaning_fields;

#[path = "template_meaning_plan.rs"]
pub(crate) mod template_meaning_plan;

#[path = "template_meanings.rs"]
pub(crate) mod template_meanings;

#[path = "template_modifier_kind.rs"]
pub(crate) mod template_modifier_kind;

#[path = "template_modifier_projection.rs"]
pub(crate) mod template_modifier_projection;

#[path = "template_number_capture.rs"]
pub(crate) mod template_number_capture;

#[path = "template_objective_plan.rs"]
pub(crate) mod template_objective_plan;

#[path = "template_output_plan.rs"]
pub(crate) mod template_output_plan;

#[path = "template_output_read.rs"]
pub(crate) mod template_output_read;

#[path = "template_plan.rs"]
pub(crate) mod template_plan;

#[path = "template_pool_binding_read.rs"]
pub(crate) mod template_pool_binding_read;

#[path = "template_projection.rs"]
pub(crate) mod template_projection;

#[path = "template_property_configuration_read.rs"]
pub(crate) mod template_property_configuration_read;

#[path = "template_property_kind.rs"]
pub(crate) mod template_property_kind;

#[path = "template_property_plan.rs"]
pub(crate) mod template_property_plan;

#[path = "template_property_projection.rs"]
pub(crate) mod template_property_projection;

#[path = "template_property_projection_read.rs"]
pub(crate) mod template_property_projection_read;

#[path = "template_property_value_check.rs"]
pub(crate) mod template_property_value_check;

#[path = "template_reference_index.rs"]
pub(crate) mod template_reference_index;

#[path = "template_rows.rs"]
pub(crate) mod template_rows;

#[path = "template_source.rs"]
pub(crate) mod template_source;

#[path = "template_source_meaning.rs"]
pub(crate) mod template_source_meaning;

#[path = "template_source_members.rs"]
pub(crate) mod template_source_members;

#[path = "template_source_read.rs"]
pub(crate) mod template_source_read;

#[path = "template_source_refs.rs"]
pub(crate) mod template_source_refs;

#[path = "template_spawn_context.rs"]
pub(crate) mod template_spawn_context;

#[path = "template_spawn_plan.rs"]
pub(crate) mod template_spawn_plan;

#[path = "template_spawn_projection.rs"]
pub(crate) mod template_spawn_projection;

#[path = "template_spawn_read.rs"]
pub(crate) mod template_spawn_read;

#[path = "template_strict_property_plan.rs"]
pub(crate) mod template_strict_property_plan;

#[path = "template_strict_property_read.rs"]
pub(crate) mod template_strict_property_read;

#[path = "template_tag_cache.rs"]
pub(crate) mod template_tag_cache;

#[path = "template_trigger_read.rs"]
pub(crate) mod template_trigger_read;

#[path = "template_value_meaning.rs"]
pub(crate) mod template_value_meaning;

#[path = "template_value_meaning_read.rs"]
pub(crate) mod template_value_meaning_read;

#[path = "template_vocabulary_read.rs"]
pub(crate) mod template_vocabulary_read;

#[path = "template_weak_plan.rs"]
pub(crate) mod template_weak_plan;

#[path = "broker_definition.rs"]
pub(crate) mod broker_definition;

#[path = "broker_flow_failure.rs"]
pub(crate) mod broker_flow_failure;

#[path = "mission_flow_owner.rs"]
pub(crate) mod mission_flow_owner;
