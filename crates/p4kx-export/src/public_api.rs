// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Public entry points and data types of the export crate.

pub use crate::action_map::{
    ACTION_MAP_PROFILE_PATH, Action, ActionMap, ActionMapError, ActionMaps, Binding, Device,
    parse_action_maps, read_action_maps,
};
pub use crate::action_map_origin::{ActionMapOrigins, action_map_summary_origins};
pub use crate::action_map_profile::ActionMapProfile;
pub use crate::action_map_write::{
    ActionMapCounts, ActionMapFormat, ActionMapLabels, write_action_maps,
    write_action_maps_with_origins,
};
pub use crate::archive_key_use::{ArchiveKeyState, ArchiveKeyUse};
pub use crate::build::{build_catalog, build_catalog_from, write_catalog, write_catalog_from};
pub use crate::built_catalog::BuiltCatalog;
pub use crate::catalog_file::CatalogFile;
pub use crate::catalog_file_write::{write_catalog_file, write_catalog_file_from};
pub use crate::catalog_output::{Compression, ZSTD_LEVEL, ZSTD_WINDOW_LOG};
pub use crate::contract::{SCHEMA_VERSION, catalog_schema};
pub use crate::crafting::{Crafting, read_crafting};
pub use crate::crafting_research::{Research, ResearchBlock, ResearchMember, ResearchValue};
pub use crate::crafting_types::{
    BlueprintPool, BlueprintTiers, Cost, CostContext, CostIndex, DismantleProcess, DismantleReturn,
    GameplayProperty, GlobalParams, ItemCost, LocationOverride, ModifierKind, PoolEntry,
    PropertyModifier, QualityCurve, QualityDistribution, QualityQuantization, QuantizationBand,
    ResourceCost, RewardChance, RewardSource, Selection, Tier, ValueRange,
};
pub use crate::crafting_warning::CraftingWarning;
pub use crate::error::CatalogError;
pub use crate::failure::CatalogFailure;
pub use crate::form::{FLAT_CSV_FILE, FLAT_NDJSON_FILE, NDJSON_FILE, RecordForm};
pub use crate::item_class::{ItemClass, ItemClassSummary};
pub use crate::localization_table::{
    LocalizationTableDatum, TableFormat, write_localization_table,
    write_observed_localization_table,
};
pub use crate::options::{CatalogOptions, CatalogPhase, CatalogProgress};
pub use crate::playability::{Playability, PlayabilityCounts, PlayabilitySummary};
pub use crate::quantization_summary::QuantizationSummary;
pub use crate::record_export::{RecordFilePlans, write_planned_record_export, write_record_export};
pub use crate::record_summary::RecordExportSummary;
pub use crate::selection::{
    EXCLUDED_TOKENS, Kind, KindRule, SHIP_EXCLUDED_TOKENS, is_kind_leftover,
};
pub use crate::ship_base::{
    BuiltShipBase, Cargo, CargoGrid, Hull, HullThreshold, Implementation, LoadoutPort, Mass,
    MassBasis, PartialPartValue, SHIP_BASE_VERSION, ShipBase, ShipBaseError, ShipBaseOptions,
    ShipReader, Source, SourceKind, SourceShare, Sourced, UnreadablePartValue, VehicleKind,
    build_ship_base, build_ship_base_from,
};
pub use crate::shop_summary::ShopSummary;
pub use crate::starmap_report::StarmapReport;
pub use crate::store_slug_summary::StoreSlugSummary;
pub use crate::summary::{
    CatalogSummary, DECOMPRESSED_LIMIT, DECOMPRESSED_WARNING_SIZE, KindSummary, LoadoutSummary,
    PaintSummary, SizeWarning, StarmapSummary, TextSummary, UPLOAD_LIMIT, UPLOAD_WARNING_SIZE,
    VariantCounts, VariantSummary,
};
pub use crate::variant_groups::{VariantGroups, read_variant_groups};
#[expect(
    deprecated,
    reason = "Public compatibility for the released Tag depth constant"
)]
pub use p4kx_extraction::MAX_TAG_DEPTH;
pub use p4kx_extraction::{
    CancelToken, ConflictPolicy, DataForge, ExtractionError, Filter, Guid, Language,
    LocalizationNote, PLACEHOLDER_KEYS, RecordExportError, RecordExportOptions, RecordFailure,
    TAG_PATH_SEPARATOR, Tag, TagResolver, TagTree, TagTreeIssues, TextSet,
};
