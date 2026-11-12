// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Tests of `p4kx-export` on a real Archive, built as one test binary; they run with `--ignored`
//! when `P4KX_DATA_P4K` names one.

mod catalog_v3_blueprint_category_tests;
mod catalog_v3_consumer;
mod catalog_v3_loot_consumer_tests;
mod catalog_v3_mission_definition_tests;
mod fuel_capacity;
#[path = "shield_generators/arena.rs"]
mod original_item_arena;
#[path = "shield_generators/numbers.rs"]
mod original_item_numbers;
#[path = "shield_generators/scalar.rs"]
mod original_item_scalar;
mod real_archive;
mod real_archive_auto_spawns;
mod real_archive_blueprints;
mod real_archive_broker_entries;
mod real_archive_canonical_texts;
mod real_archive_catalog_links;
mod real_archive_catalog_ships;
mod real_archive_catalog_tags;
mod real_archive_commodity_refining;
mod real_archive_compression;
mod real_archive_contract_cooldown_overrides;
mod real_archive_contract_cooldowns;
mod real_archive_contract_facts;
mod real_archive_contract_gaps;
mod real_archive_contract_template_parts;
mod real_archive_contract_templates;
mod real_archive_contracts;
mod real_archive_crafters;
mod real_archive_crafting;
mod real_archive_crafting_catalog;
mod real_archive_crafting_details;
mod real_archive_explosive_components;
mod real_archive_faction_reputation_source;
mod real_archive_factions;
mod real_archive_flow_action_wrappers;
mod real_archive_gear_stats;
mod real_archive_gear_trace;
mod real_archive_grade;
mod real_archive_hacking_components;
mod real_archive_hauling;
mod real_archive_insurance;
mod real_archive_item_alignment;
mod real_archive_item_components;
mod real_archive_item_durability;
mod real_archive_item_recovery;
mod real_archive_item_stats;
mod real_archive_items;
mod real_archive_leftovers;
mod real_archive_loadouts;
mod real_archive_loot;
mod real_archive_mining;
mod real_archive_mining_stats;
mod real_archive_npc_spawns;
mod real_archive_paints;
mod real_archive_playability;
mod real_archive_property_sets;
mod real_archive_quantization;
mod real_archive_quantum_drives;
mod real_archive_records;
mod real_archive_reputation;
mod real_archive_reputation_perks;
mod real_archive_reputation_sandbox;
mod real_archive_reputation_standings;
mod real_archive_resource_components;
mod real_archive_resource_network;
mod real_archive_resource_oracles;
mod real_archive_ship_base;
mod real_archive_ship_matrix;
mod real_archive_shop_inventory_v3;
mod real_archive_shop_items;
mod real_archive_shops;
mod real_archive_spawns;
mod real_archive_starmap;
mod real_archive_store_slugs;
mod real_archive_tags;
mod real_archive_text_oracle;
mod real_archive_texts;
mod real_archive_variant_groups;
mod real_archive_variant_keys;
mod real_archive_variants;
mod real_archive_vehicle_armor;
mod real_archive_verification;
mod real_archive_weapon_actions;
mod shield_generators;
mod ship_matrix_compare;

#[path = "../it/resource_density_schema_fixture.rs"]
mod resource_density_schema_fixture;

#[path = "../it/resource_fixture.rs"]
mod resource_fixture;

#[path = "../it/resource_fixture_values.rs"]
mod resource_fixture_values;

#[path = "../it/resource_quality_schema_fixture.rs"]
mod resource_quality_schema_fixture;

#[path = "../it/resource_schema_fixture.rs"]
mod resource_schema_fixture;

mod catalog_v3_consumer_origins_tests;
mod catalog_v3_consumer_tests;
#[path = "../it/commodity_v3_fixture.rs"]
mod commodity_v3_fixture;
#[path = "../it/commodity_v3_fixture_records.rs"]
mod commodity_v3_fixture_records;
#[path = "../it/commodity_v3_fixture_schema.rs"]
mod commodity_v3_fixture_schema;

mod real_archive_resource_topology;
