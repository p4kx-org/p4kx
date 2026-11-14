// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Export service of P4KX: the Record export and the Basetool catalog (ADR-0002).
//!
//! [`write_record_export`] writes a selection of Records in a [`RecordForm`]: one Record XML or
//! Record JSON file per Record under its record path (ADR-0008), or one NDJSON file with one
//! Record per line in record-path order, and returns a [`RecordExportSummary`]. Record JSON is
//! defined in `docs/record-json.md` and `schemas/record-json.schema.json`.
//!
//! [`build_catalog`] selects Records by path and type, excludes development leftovers, resolves
//! English/German texts, and writes deterministic UTF-8 JSON with crafting, [`ItemClass`] and
//! [`Playability`] data. Normal publication refuses incomplete source reads or projections.
//! [`CatalogPublicationMode::DiagnosticPartial`] retains readable results with explicit health
//! and failure evidence. [`catalog_schema`] generates the schema from the same catalog types
//! (`docs/basetool-catalog.md`).
//!
//! The starmap objects of the catalog take their world positions from the system containers
//! (`ObjectContainers/PU/system`): the position of every `Child` element is composed with the
//! positions and rotations of the Children above it ([`StarmapSummary`] counts the result).
//!
//! The harvestable locations of the catalog are the system containers whose `.soc` names a
//! provider preset, each with the starmap object it hangs below in that walk (ADR-0236).
//!
//! The shops of the catalog are the `ShopBackup` entries of the registry
//! `Data\Libs\CosmoTools\LayerRecovery\LayerBackups.xml`, with kind and brand from the shop
//! containers and a link to the starmap object of their location ([`ShopSummary`] counts the
//! result, ADR-0069) and the item GUIDs of the inventory file their containers name (ADR-0071).
//!
//! [`build_ship_base`] reads the base data of every ship: hull mass and hit points from the
//! vehicle implementation, cargo capacity from the cargo grids of the default Loadout, size and
//! the Loadout itself, each value with its source file and field (`docs/ship-base-data.md`);
//! [`ShipReader`] reads one ship.
//!
//! [`read_variant_groups`] finds the variant groups of the ships and items (ADR-0066) with the
//! rule of the catalog's `variantOf` and without building a catalog, for a front-end that folds
//! variants under their base.
//!
//! [`TagTree::build`] reads the selected tag paths and their hierarchy from a [`DataForge`].
//! [`TagTree::find_path`] finds Tags by path. Catalog kinds such as paints and loadouts use this tree.
//! `RecordExportOptions::tags` adds the tag path to every tag Reference of an exported Record.
//!
//! [`read_crafting`] reads the complete crafting data of a DataForge as [`Crafting`]: every tier
//! of every blueprint with its selections ("choose n"), quantity multipliers and
//! quality modifiers, the quality distributions with their location overrides, the quality
//! quantizations with the `ResourceType`s that use them, the blueprint
//! pools with the contracts that reward them, the dismantle processes and the global
//! parameters. The catalog keeps its quality declarations as canonical source data.
//!
//! [`read_action_maps`] reads the action maps of the default profile
//! (`Data\Libs\Config\defaultProfile.xml`): every action with the localisation key of its label
//! and its default bindings per [`Device`]; [`write_action_maps`] writes them with the labels in
//! one language as JSON or CSV.
//!
//! [`write_localization_table`] writes every localisation key of a [`TextSet`] with its texts in
//! the chosen languages as JSON or CSV.

mod action_map;
mod action_map_origin;
mod action_map_profile;
mod action_map_source;
#[cfg(test)]
mod action_map_tests;
mod action_map_write;
mod archive_key_use;
mod blueprint;
mod blueprint_backfill_source;
mod blueprint_catalog_read;
mod blueprint_catalog_rows;
mod blueprint_category;
mod blueprint_choice_inputs;
mod blueprint_context_contract;
mod blueprint_context_read;
mod blueprint_definition;
mod blueprint_field_source;
mod blueprint_ingredient_read;
mod blueprint_presentation;
mod blueprint_research_contract;
mod blueprint_research_read;
mod blueprint_rule_inputs;
mod blueprint_tier_contract;
mod blueprint_tier_read;
mod broker_canonical_read;
mod broker_entry;
mod broker_entry_read;
mod broker_projection_read;
mod broker_reputation_read;
mod broker_standing_read;
mod broker_terms;
mod broker_values;
mod build;
mod build_derived;
mod build_harvest;
mod build_loot;
mod build_starmap;
mod built_catalog;
mod cargo_manifest;
mod catalog_crafting;
mod catalog_file;
mod catalog_file_write;
mod catalog_output;
mod catalog_reference;
mod catalog_reference_health;
mod catalog_reference_texts;
mod catalog_row;
mod catalog_rows;
mod catalog_target_index;
mod catalog_template_modules;
mod catalog_v3;
mod collected;
#[cfg(test)]
mod collected_tests;
mod condition_schema;
mod container_read;
mod contract;
mod contract_cargo;
mod contract_completeness;
mod contract_crafter;
mod contract_entities;
mod contract_facts;
mod contract_harvest;
mod contract_harvest_config;
mod contract_hauling;
mod contract_insurance;
mod contract_loadout;
mod contract_loot;
mod contract_loot_legacy;
mod contract_mining;
mod contract_mining_params;
mod contract_mission_data;
mod contract_missions;
mod contract_offer;
mod contract_recovery;
mod contract_reward;
mod contract_rewards;
mod contract_shops;
mod contract_source;
mod contract_starmap;
mod contract_stats;
mod contract_stats_gear;
mod contract_stats_mining;
mod contract_store_slugs;
mod contract_template_read;
mod crafter;
mod crafter_attach;
mod crafter_fields;
mod crafter_hosts;
mod crafter_queue;
mod crafter_rules;
mod crafting;
mod crafting_cost_read;
mod crafting_quantization;
mod crafting_recipe_costs;
mod crafting_records;
mod crafting_research;
mod crafting_scan;
mod crafting_sources;
mod crafting_tier;
mod crafting_types;
mod crafting_warning;
mod definition_reference;
mod digest;
mod entity_selection;
mod entity_spawn_model;
mod entry_source;
mod error;
mod export_archive_health;
mod export_builder_capture;
mod export_dependency_read;
mod export_evidence_lock;
mod export_field_collect;
mod export_field_error;
mod export_field_inspection;
mod export_field_walk;
mod export_projection_fields;
mod export_source_failure;
mod export_tag_read;
mod failure;
mod form;
mod guid_text;
mod harvest_config;
mod harvest_link;
mod harvest_locations;
mod harvest_preset;
mod harvest_source;
mod item;
mod item_class;
mod item_stats;
mod item_variant;
mod line_writer;
mod loadout;
mod loadout_port;
mod localization_table;
mod location_template;
mod loot_container_source;
mod loot_containers;
mod loot_entry;
mod loot_entxml;
mod loot_kinds;
mod loot_legacy;
mod loot_records;
mod loot_soc;
mod loot_soc_source;
mod loot_unreadable;
mod loot_walk;
mod manufacturer;
mod memo;
mod mining;
mod mining_params;
mod mission_blueprints;
mod mission_cooldown;
mod mission_data;
mod mission_entry_source;
mod mission_facts;
mod mission_generator;
mod mission_giver;
mod mission_hauling;
mod mission_hauling_levels;
mod mission_index;
mod mission_intros;
mod mission_kinds;
mod mission_layers;
mod mission_ledger;
mod mission_places;
mod mission_pool_ledger;
mod mission_property_auto;
mod mission_property_auto_read;
mod mission_property_clue;
mod mission_property_clue_read;
mod mission_property_fields;
mod mission_property_investigation;
mod mission_property_investigation_read;
mod mission_property_name;
mod mission_property_name_read;
mod mission_property_option;
mod mission_property_option_read;
mod mission_property_race;
mod mission_property_set;
mod mission_property_set_index;
mod mission_property_set_registry;
mod mission_property_source;
mod mission_property_sources;
mod mission_property_value_read;
mod mission_results;
mod mission_shared;
mod mission_siblings;
mod mission_standing;
mod mission_text_inputs;
mod mission_title;
mod mission_title_origin;
mod mission_titles;
mod missions;
mod ndjson;
mod ndjson_output;
mod npc_spawn_model;
mod npc_spawn_read;
mod options;
mod owned_reference;
mod owned_reference_read;
mod paint;
mod paint_ledger;
mod paint_port;
mod picked;
mod playability;
mod playability_rules;
mod public_api;
mod quality_catalog;
mod quality_curve;
mod quality_curve_read;
mod quantization_summary;
mod record_derived_observation;
mod record_export;
mod record_fields;
mod record_observation;
mod record_read;
mod record_source_status;
mod record_summary;
mod recovery;
mod scan;
mod scan_health;
mod scan_signature;
#[cfg(test)]
mod schema_control_tests;
mod schema_fields;
mod schema_vocabulary;
mod scoped_location;
mod scoped_location_fields;
mod scoped_location_link;
mod scoped_location_read;
mod scoped_location_weak;
mod selection;
mod selection_index;
mod selection_lookup;
mod selection_rules;
mod ship;
mod ship_base;
mod ship_pool;
mod ship_store_slugs;
mod shop_container_scan;
mod shop_containers;
mod shop_entry_read;
mod shop_inventories;
mod shop_inventory_compatibility;
mod shop_inventory_fields;
mod shop_inventory_file;
mod shop_inventory_observations;
mod shop_inventory_resolution;
mod shop_inventory_source;
mod shop_inventory_source_read;
mod shop_links;
#[cfg(test)]
mod shop_observation_tests;
mod shop_prefabs;
mod shop_registry;
mod shop_soc;
mod shop_summary;
mod shops;
mod soc_preset;
mod source_fields;
mod source_pointer;
mod source_projection_notes;
mod spawn_error;
mod spawn_layers;
mod spawn_model;
mod spawn_pools;
mod spawn_read;
mod starmap;
mod starmap_containers;
mod starmap_index;
mod starmap_parse;
mod starmap_read;
mod starmap_report;
#[cfg(test)]
mod starmap_tests;
mod starmap_transform;
mod starmap_walk;
mod starmap_xml;
mod store_slug_summary;
mod summary;
mod template_extras;
pub(crate) use catalog_template_modules::{
    broker_definition, broker_flow_failure, catalog_configurations, location_condition_kind,
    mission_flow_owner, scoped_location_kind, scoped_location_shape, spawn_tag_text,
    template_action_meaning, template_action_read, template_binding_read, template_canonical_read,
    template_condition_read, template_configuration, template_context_error,
    template_context_owner_read, template_context_read, template_context_source_read,
    template_context_tag_read, template_context_value, template_context_weak_read,
    template_declaration_binding, template_definition, template_dependency_spawns,
    template_derived_failures, template_field_binding, template_field_capture,
    template_flow_configuration_read, template_flow_plan, template_hauling_projection,
    template_hauling_read, template_input, template_input_read, template_localization_plan,
    template_localization_read, template_meaning_fields, template_meaning_plan, template_meanings,
    template_modifier_kind, template_modifier_projection, template_number_capture,
    template_objective_plan, template_output_plan, template_output_read, template_plan,
    template_pool_binding_read, template_projection, template_property_configuration_read,
    template_property_kind, template_property_plan, template_property_projection,
    template_property_projection_read, template_property_value_check, template_reference_index,
    template_rows, template_source, template_source_meaning, template_source_members,
    template_source_read, template_source_refs, template_spawn_context, template_spawn_plan,
    template_spawn_projection, template_spawn_read, template_strict_property_plan,
    template_strict_property_read, template_tag_cache, template_trigger_read,
    template_value_meaning, template_value_meaning_read, template_vocabulary_read,
    template_weak_plan,
};
mod template_issues;
mod template_modifier;
mod template_objective;
mod template_property;
mod template_property_read;
mod texts;
mod variant_groups;
mod variants;

pub(crate) use catalog_v3::export_health_capture::{
    CatalogReadFailure, catalog_failures, catalog_read_failure, export_diagnostic,
    export_entry_counts, export_entry_ledger, export_health, export_health_aggregate,
    export_health_api, export_health_compile, export_health_derived, export_health_entries,
    export_health_fields, export_health_prepare, export_health_prepared, export_health_records,
    export_health_warning, export_ledger, export_reference_issue, failure_text,
    health_diagnostic_observation, health_example_samples, health_failed_observation,
    health_metadata, health_text_view,
};
pub(crate) use catalog_v3::{
    canonical_record_header, catalog_definitions, catalog_number_capture,
    catalog_number_diagnostics, catalog_number_health, catalog_origins, catalog_projection,
    catalog_projections, catalog_read, catalog_record_reference, catalog_table,
    catalog_text_contract, commodity, commodity_authoring_contract, commodity_authoring_inputs,
    commodity_authoring_read, commodity_backfill, commodity_binding_read, commodity_contract,
    commodity_inputs, commodity_presentation, commodity_resource, commodity_resource_presentation,
    commodity_source, commodity_text_choice, commodity_text_input, number_position,
    quality_bands_read, quality_curve_source_read, quality_source, quality_source_read,
    record_header_field, resource_density_read, resource_density_source, resource_float,
    resource_group, resource_group_read, resource_integer, resource_memberships, resource_number,
    resource_property_contract, resource_property_read, resource_raw_policy, resource_read,
    resource_type_contract, resource_value_read, source_header, source_value_catalog,
};

pub use export_health_api::*;
pub use public_api::*;

mod shop_container_xml;
mod shop_inventory_name;
mod shop_inventory_path;
