// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Export service of P4KX: the Record export and the Basetool catalog (ADR-0002).
//!
//! [`write_record_export`] writes a selection of Records in a [`RecordForm`]: one Record XML or
//! Record JSON file per Record under its record path (ADR-0008), or one NDJSON file with one
//! Record per line in record-path order, and returns a [`RecordExportSummary`]. Record JSON is
//! defined in `docs/record-json.md` and `schemas/record-json.schema.json`.
//!
//! [`build_catalog`] reads the DataForge of an open Archive, selects the Records of every kind by
//! record-path prefix and type in parallel, leaves out development leftovers, resolves names and
//! descriptions in English and German from the Archive's `global.ini` files, takes the blueprints
//! with all tiers, the quality, pool, reward-source and dismantle Records from [`read_crafting`],
//! reads each item's
//! [`ItemClass`] from its English description, classifies every ship and item as a
//! [`Playability`] and writes the
//! catalog as deterministic UTF-8 JSON. A selected Record that cannot be read is left out and
//! reported as a [`CatalogFailure`] in the [`CatalogSummary`]. [`catalog_schema`] returns its JSON Schema, generated from
//! the same types (`docs/basetool-catalog.md`).
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
//! [`TagTree::build`] reads the tag tree of a [`DataForge`]: tag GUID to tag path, parent and
//! children, at most [`MAX_TAG_DEPTH`] levels deep. [`TagTree::find_path`] finds the tags with a
//! tag path. The catalog kinds that match tags (paints, loadouts) use the tree, and
//! `RecordExportOptions::tags` adds the tag path to every tag Reference of an exported Record.
//!
//! [`read_crafting`] reads the complete crafting data of a DataForge as [`Crafting`]: every tier
//! of every blueprint with its selections ("choose n"), quantity multipliers and
//! quality modifiers, the quality distributions with their location overrides, the quality
//! quantizations with the `ResourceType`s that use them, the blueprint
//! pools with the contracts that reward them, the dismantle processes and the global
//! parameters. It is the library API the catalog's crafting fields are built on.
//!
//! [`read_action_maps`] reads the action maps of the default profile
//! (`Data\Libs\Config\defaultProfile.xml`): every action with the localisation key of its label
//! and its default bindings per [`Device`]; [`write_action_maps`] writes them with the labels in
//! one language as JSON or CSV.
//!
//! [`write_localization_table`] writes every localisation key of a [`TextSet`] with its texts in
//! the chosen languages as JSON or CSV.

mod action_map;
#[cfg(test)]
mod action_map_tests;
mod action_map_write;
mod blueprint;
mod blueprint_category;
mod broker_entry;
mod broker_entry_read;
mod broker_identity;
mod broker_links;
mod broker_requirements;
mod broker_terms;
mod broker_values;
mod build;
mod build_derived;
mod build_harvest;
mod build_loot;
mod build_starmap;
mod cargo_manifest;
mod catalog_crafting;
mod catalog_file;
mod catalog_output;
mod catalog_recipe;
mod closed_set;
mod collected;
#[cfg(test)]
mod collected_tests;
mod commodity;
mod container_read;
mod contract;
mod contract_cargo;
mod contract_crafter;
mod contract_crafting;
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
mod contract_quantization;
mod contract_recipe;
mod contract_recovery;
mod contract_reputation;
mod contract_reward;
mod contract_rewards;
mod contract_shops;
mod contract_source;
mod contract_starmap;
mod contract_stats;
mod contract_stats_gear;
mod contract_stats_mining;
mod contract_store_slugs;
mod contract_template;
mod contract_template_read;
mod crafter;
mod crafter_attach;
mod crafter_fields;
mod crafter_hosts;
mod crafter_queue;
mod crafter_rules;
mod crafting;
mod crafting_quantization;
mod crafting_records;
mod crafting_scan;
mod crafting_sources;
mod crafting_tier;
mod crafting_types;
mod crafting_warning;
mod digest;
mod entity_selection;
mod error;
mod failure;
mod form;
mod guid_text;
mod harvest_config;
mod harvest_link;
mod harvest_locations;
mod harvest_preset;
mod item;
mod item_class;
mod item_stats;
mod item_variant;
mod line_writer;
mod loadout;
mod loadout_port;
mod localization_table;
mod location_template;
mod loot_containers;
mod loot_entry;
mod loot_entxml;
mod loot_kinds;
mod loot_legacy;
mod loot_records;
mod loot_soc;
mod loot_unreadable;
mod loot_walk;
mod manufacturer;
mod memo;
mod mining;
mod mining_params;
mod mission_blueprints;
mod mission_cooldown;
mod mission_data;
mod mission_facts;
mod mission_giver;
mod mission_hauling;
mod mission_hauling_levels;
mod mission_index;
mod mission_intros;
mod mission_kinds;
mod mission_layers;
mod mission_places;
mod mission_results;
mod mission_shared;
mod mission_siblings;
mod mission_standing;
mod mission_title;
mod mission_titles;
mod missions;
mod ndjson;
mod options;
mod paint;
mod paint_port;
mod picked;
mod playability;
mod playability_rules;
mod quantization_summary;
mod record_export;
mod record_fields;
mod record_read;
mod record_summary;
mod recovery;
mod reputation;
mod resource_group;
mod scan;
mod scan_signature;
mod selection;
mod selection_index;
mod selection_rules;
mod ship;
mod ship_base;
mod ship_pool;
mod ship_store_slugs;
mod shop_container_scan;
mod shop_containers;
mod shop_inventories;
mod shop_links;
mod shop_prefabs;
mod shop_registry;
mod shop_soc;
mod shop_summary;
mod shops;
mod soc_preset;
mod spawn_layers;
mod spawn_model;
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
mod template_flow;
mod template_issues;
mod template_modifier;
mod template_objective;
mod template_objective_read;
mod template_property;
mod template_property_read;
mod texts;
mod variant_groups;
mod variants;

pub use action_map::{
    ACTION_MAP_PROFILE_PATH, Action, ActionMap, ActionMapError, ActionMaps, Binding, Device,
    parse_action_maps, read_action_maps,
};
pub use action_map_write::{ActionMapCounts, ActionMapFormat, write_action_maps};
pub use build::{build_catalog, build_catalog_from, write_catalog, write_catalog_from};
pub use catalog_file::CatalogFile;
pub use catalog_output::{Compression, ZSTD_LEVEL, ZSTD_WINDOW_LOG};
pub use contract::{SCHEMA_VERSION, catalog_schema};
pub use crafting::{Crafting, read_crafting};
pub use crafting_types::{
    BlueprintPool, BlueprintTiers, Cost, CostContext, DismantleProcess, DismantleReturn,
    GameplayProperty, GlobalParams, ItemCost, LocationOverride, ModifierKind, PoolEntry,
    PropertyModifier, QualityCurve, QualityDistribution, QualityQuantization, QuantizationBand,
    ResourceCost, RewardChance, RewardSource, Selection, Tier, ValueRange,
};
pub use crafting_warning::CraftingWarning;
pub use error::CatalogError;
pub use failure::CatalogFailure;
pub use form::{FLAT_CSV_FILE, FLAT_NDJSON_FILE, NDJSON_FILE, RecordForm};
pub use item_class::{ItemClass, ItemClassSummary};
pub use localization_table::{TableFormat, write_localization_table};
pub use options::{CatalogOptions, CatalogPhase, CatalogProgress};
pub use p4kx_extraction::{
    CancelToken, ConflictPolicy, DataForge, ExtractionError, Filter, Guid, Language,
    LocalizationNote, MAX_TAG_DEPTH, PLACEHOLDER_KEYS, RecordExportError, RecordExportOptions,
    RecordFailure, TAG_PATH_SEPARATOR, Tag, TagResolver, TagTree, TagTreeIssues, TextSet,
};
pub use playability::{Playability, PlayabilityCounts, PlayabilitySummary};
pub use quantization_summary::QuantizationSummary;
pub use record_export::{RecordFilePlans, write_planned_record_export, write_record_export};
pub use record_summary::RecordExportSummary;
pub use selection::{EXCLUDED_TOKENS, Kind, KindRule, SHIP_EXCLUDED_TOKENS};
pub use ship_base::{
    BuiltShipBase, Cargo, CargoGrid, Hull, HullThreshold, Implementation, LoadoutPort, Mass,
    MassBasis, PartialPartValue, SHIP_BASE_VERSION, ShipBase, ShipBaseError, ShipBaseOptions,
    ShipReader, Source, SourceKind, SourceShare, Sourced, UnreadablePartValue, VehicleKind,
    build_ship_base, build_ship_base_from,
};
pub use shop_summary::ShopSummary;
pub use starmap_report::StarmapReport;
pub use store_slug_summary::StoreSlugSummary;
pub use summary::{
    BuiltCatalog, CatalogSummary, DECOMPRESSED_LIMIT, DECOMPRESSED_WARNING_SIZE, KindSummary,
    LoadoutSummary, PaintSummary, SizeWarning, StarmapSummary, TextSummary, UPLOAD_LIMIT,
    UPLOAD_WARNING_SIZE, VariantCounts, VariantSummary,
};
pub use variant_groups::{VariantGroups, read_variant_groups};
