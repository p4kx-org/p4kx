// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::sync::Arc;
use std::time::SystemTime;

use p4kx_assembly::{ArchiveFiles, Entities};
use p4kx_channels::{PublicPatches, channel_name};
use p4kx_extraction::{Archive, CancelToken, DataForge, DataForgeEntry, utc_rfc3339};

use crate::build_derived::{add_contracts, add_loadouts, add_paints};
use crate::build_harvest::add_harvest_locations;
use crate::build_loot::add_loot_containers;
use crate::build_starmap::add_starmap;
use crate::catalog_crafting::{CraftingKinds, kinds};
use crate::catalog_output::{Compression, emit};
use crate::collected::Collected;
use crate::contract::{
    BuildSource, Catalog, Completeness, KindCompleteness, SCHEMA_VERSION, Source,
};
use crate::crafting::read_with_progress;
use crate::crafting_warning::CraftingWarning;
use crate::mission_data::link_scenario_factions;
use crate::recovery::{Recovery, read_recovery};
use crate::scan::Scan;
use crate::ship_store_slugs::StoreSlugs;
use crate::shops::add_shops;
use crate::starmap_containers::Placements;
use crate::starmap_report::StarmapReport;
use crate::texts::{TextSources, Texts};
use crate::{
    BuiltCatalog, CatalogError, CatalogFailure, CatalogOptions, CatalogSummary, LoadoutSummary,
    PaintSummary, ShopSummary, StarmapSummary, StoreSlugSummary,
};

/// Builds the Basetool catalog of the DataForge in `archive`.
///
/// Every Record is checked in parallel: a kind takes the Records under one of its record-path
/// prefixes whose struct is its type and, with a Filter, whose record path the Filter matches;
/// of those, development leftovers ([`EXCLUDED_TOKENS`](crate::EXCLUDED_TOKENS), and for ships
/// [`SHIP_EXCLUDED_TOKENS`](crate::SHIP_EXCLUDED_TOKENS)) are counted as
/// skipped. Names and descriptions resolve in the English and German `global.ini`; a key or a
/// language the Archive lacks leaves that text `null` and is counted in the summary. An item's
/// class is read from the `Class:` line of its English description, a commodity's missing texts
/// from its `ResourceType` Record, and a blueprint's ingredients from the costs of its first
/// tier. A ship's and an item's [`Playability`](crate::Playability) is decided by structural
/// rules (`docs/basetool-catalog.md`). A `ResourceType` Record that a cost of any tier or a kept
/// quality quantization uses and no commodity Record carries becomes a commodity of its own. A
/// resource group takes its parent from
/// the group that lists it, a blueprint category only its Record. Entities are ordered by the
/// text of their GUID, so equal input and export time give byte-identical JSON. A selected
/// Record that cannot be read is left out and listed in
/// [`CatalogSummary::failures`].
///
/// # Errors
///
/// Returns [`CatalogError::Load`] or [`CatalogError::DataForge`] when the DataForge cannot be
/// read, [`CatalogError::LoadTexts`] or [`CatalogError::Texts`] when a `global.ini` the Archive
/// holds cannot be read, and [`CatalogError::Cancelled`] once the [`CancelToken`] of `options`
/// is cancelled.
pub fn build_catalog(
    archive: &Archive,
    options: &CatalogOptions<'_>,
) -> Result<BuiltCatalog, CatalogError> {
    if options.cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let entry = DataForgeEntry::read(archive).map_err(CatalogError::Load)?;
    build_catalog_from(archive, &entry, options)
}

/// Builds the Basetool catalog like [`build_catalog`] and streams it like [`write_catalog_from`].
///
/// # Errors
///
/// As [`build_catalog`] and [`write_catalog_from`].
pub fn write_catalog<W: Write>(
    archive: &Archive,
    options: &CatalogOptions<'_>,
    compression: Compression,
    create: impl FnOnce() -> io::Result<W>,
) -> Result<(CatalogSummary, W), CatalogError> {
    if options.cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let entry = DataForgeEntry::read(archive).map_err(CatalogError::Load)?;
    write_catalog_from(archive, &entry, options, compression, create)
}

/// Builds the Basetool catalog like [`build_catalog`] from `entry`, the DataForge Entry already
/// read from `archive`, so that a caller holding it does not read it again.
///
/// # Errors
///
/// As [`build_catalog`], except that the DataForge Entry is not read here.
pub fn build_catalog_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    options: &CatalogOptions<'_>,
) -> Result<BuiltCatalog, CatalogError> {
    let (parts, written) = assemble(archive, entry, options, |catalog| {
        emit(catalog, Vec::new(), Compression::None, options.cancel)
    })?;
    Ok(BuiltCatalog {
        summary: parts.summary(written.size, written.compressed),
        json: written.sink,
    })
}

/// Builds the Basetool catalog like [`build_catalog_from`] and streams its JSON into the writer
/// `create` returns, compressed as `compression` says.
///
/// The JSON is never held in memory; zstd serialises it twice, once to count its length for the
/// frame header.
///
/// `create` is called once, after the catalog is built and before the first byte is written, so a
/// build that fails or is cancelled has created nothing. Returns the summary, whose
/// [`size`](CatalogSummary::size) is the JSON size and whose
/// [`compressed_size`](CatalogSummary::compressed_size) is the size written when compressed, and
/// the writer, flushed and with the compressed frame finished.
///
/// # Errors
///
/// As [`build_catalog`], except that the DataForge Entry is not read here, plus
/// [`CatalogError::Write`] when `create` or the writer fails and [`CatalogError::Cancelled`] when
/// the [`CancelToken`] of `options` is cancelled while writing.
pub fn write_catalog_from<W: Write>(
    archive: &Archive,
    entry: &DataForgeEntry,
    options: &CatalogOptions<'_>,
    compression: Compression,
    create: impl FnOnce() -> io::Result<W>,
) -> Result<(CatalogSummary, W), CatalogError> {
    let (parts, written) = assemble(archive, entry, options, |catalog| {
        let out = create().map_err(CatalogError::Write)?;
        emit(catalog, out, compression, options.cancel)
    })?;
    Ok((
        parts.summary(written.size, written.compressed),
        written.sink,
    ))
}

/// What a build knows of a catalog besides its size.
pub(crate) struct Parts {
    kinds: Vec<crate::KindSummary>,
    texts: Vec<crate::TextSummary>,
    notes: Vec<p4kx_extraction::LocalizationNote>,
    item_classes: crate::ItemClassSummary,
    paints: PaintSummary,
    loadouts: LoadoutSummary,
    starmap: StarmapSummary,
    starmap_report: Box<StarmapReport>,
    shops: Box<ShopSummary>,
    store_slugs: Box<StoreSlugSummary>,
    playability: crate::PlayabilitySummary,
    variants: crate::VariantSummary,
    quantizations: crate::QuantizationSummary,
    failures: Vec<CatalogFailure>,
    warnings: Vec<CraftingWarning>,
}

impl Parts {
    fn summary(self, size: u64, compressed_size: Option<u64>) -> CatalogSummary {
        CatalogSummary {
            kinds: self.kinds,
            texts: self.texts,
            notes: self.notes,
            item_classes: self.item_classes,
            paints: self.paints,
            loadouts: self.loadouts,
            starmap: self.starmap,
            starmap_report: self.starmap_report,
            shops: self.shops,
            store_slugs: self.store_slugs,
            playability: self.playability,
            variants: self.variants,
            quantizations: self.quantizations,
            failures: self.failures,
            warnings: self.warnings,
            size,
            compressed_size,
        }
    }
}

/// Builds the catalog and hands it to `emit`, which serialises it.
#[expect(
    clippy::too_many_lines,
    reason = "one step and one field per kind of the catalog"
)]
fn assemble<R>(
    archive: &Archive,
    entry: &DataForgeEntry,
    options: &CatalogOptions<'_>,
    emit: impl FnOnce(&Catalog<'_>) -> Result<R, CatalogError>,
) -> Result<(Parts, R), CatalogError> {
    if options.cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let forge = entry.open().map_err(CatalogError::DataForge)?;
    let sources = TextSources::read(archive)?;
    let texts = Texts::parse(&sources)?;
    let crafting = kinds(
        read_with_progress(&forge, options.filter, options.cancel, options.progress)?,
        &texts,
        options.filter,
    );
    let files = ArchiveFiles::new(archive);
    let entities = Arc::new(Entities::new(&forge));
    let (mut collected, placements) = scan(
        archive, &forge, &files, &entities, options, &texts, crafting,
    )?;
    let paint_summary = add_paints(&forge, &mut collected, options.cancel)?;
    let loadout_summary = add_loadouts(archive, &entities, &mut collected, options.cancel)?;
    let variants = collected.group_variants();
    let starmap_summary = add_starmap(&mut collected, placements.counts);
    add_harvest_locations(archive, &placements, &mut collected, options.cancel)?;
    add_loot_containers(archive, &placements, &mut collected, options.cancel)?;
    add_contracts(&forge, &texts, options, &mut collected)?;
    link_scenario_factions(&forge, &mut collected.mission_kinds.scenarios);
    let shop_summary = add_shops(archive, &files, &mut collected, options)?;
    let Recovery { rules, failures } = read_recovery(&forge, options.filter);
    collected.add_failures(failures);
    let kinds = collected.summary(options.filter.is_none());
    let store_slugs = StoreSlugs::embedded()?;
    let resolved = store_slugs.resolve(
        collected
            .ships
            .iter()
            .map(|ship| (ship.guid.0, ship.class_name)),
        options.filter.is_some(),
    );
    let catalog = Catalog {
        schema_version: SCHEMA_VERSION,
        source: source(archive, options.exported_at, options.public_patches),
        completeness: Completeness::of_every_kind(completeness_of(options)),
        manufacturers: collected.manufacturers,
        items: collected.items,
        ships: collected.ships,
        commodities: collected.commodities,
        blueprints: collected.blueprints,
        resource_groups: collected.resource_groups,
        blueprint_categories: collected.blueprint_categories,
        gameplay_properties: collected.gameplay_properties,
        quality_distributions: collected.quality_distributions,
        quality_quantizations: collected.quality_quantizations,
        blueprint_pools: collected.blueprint_pools,
        dismantle_processes: collected.dismantle_processes,
        crafting_global: collected.crafting_global,
        paints: collected.paints,
        loadouts: collected.loadouts,
        starmap_objects: collected.starmap_objects,
        contracts: collected.contracts,
        mineable_elements: collected.mineable_elements,
        refining_processes: collected.refining_processes,
        mineable_compositions: collected.mineable_compositions,
        mining_global_params: collected.mining_global_params,
        harvestable_provider_presets: collected.harvestable_provider_presets,
        sub_harvestable_multi_configs: collected.sub_harvestable_multi_configs,
        harvestable_locations: collected.harvestable_locations,
        loot_containers: collected.loot_containers,
        shops: collected.shops,
        faction_reputations: collected.mission_kinds.factions,
        reputation_scopes: collected.mission_kinds.scopes,
        reputation_standings: collected.mission_kinds.standings,
        reputation_contexts: collected.mission_kinds.contexts,
        mission_types: collected.mission_kinds.types,
        mission_localities: collected.mission_kinds.localities,
        contract_scenarios: collected.mission_kinds.scenarios,
        loot_tables: collected.loot_kinds.tables,
        loot_archetypes: collected.loot_kinds.archetypes,
        loot_pool_filters: collected.loot_kinds.filters,
        legacy_loot_tables: collected.loot_kinds.legacy_tables,
        legacy_loot_archetypes: collected.loot_kinds.legacy_archetypes,
        mineable_entities: collected.mineable_entities,
        crafters: collected.crafters,
        cargo_manifests: collected.mission_kinds.manifests,
        mission_location_templates: collected.mission_location_templates,
        contract_ship_spawns: collected.contract_ship_spawns,
        contract_location_queries: collected.contract_location_queries,
        contract_templates: collected.contract_templates,
        ship_store_slugs: resolved.exported,
        item_recovery: rules,
    };
    let output = emit(&catalog)?;
    Ok((
        Parts {
            kinds,
            texts: texts.summary(),
            notes: texts.notes().to_vec(),
            item_classes: collected.item_classes,
            paints: paint_summary,
            loadouts: loadout_summary,
            starmap: starmap_summary,
            starmap_report: Box::new(placements.report),
            shops: Box::new(shop_summary),
            store_slugs: Box::new(resolved.summary),
            playability: collected.playability,
            variants,
            quantizations: collected.quantization_summary,
            failures: collected.failures,
            warnings: collected.warnings,
        },
        output,
    ))
}

/// Reads the system containers, checks every Record of `forge` and sorts what the kinds selected;
/// the containers are read when a starmap object or a provider preset is wanted.
fn scan<'f>(
    archive: &Archive,
    forge: &'f DataForge<'f>,
    files: &'f ArchiveFiles<'f>,
    entities: &Arc<Entities<'f>>,
    options: &CatalogOptions<'_>,
    texts: &Texts<'f>,
    crafting: CraftingKinds<'f>,
) -> Result<(Collected<'f>, Placements), CatalogError> {
    let rules = [
        crate::Kind::StarmapObjects.records(),
        crate::Kind::HarvestableProviderPresets.records(),
    ];
    let wanted = forge.records().any(|record| {
        record.record_path().is_ok_and(|path| {
            rules.iter().any(|rule| rule.matches_path(path))
                && options.filter.is_none_or(|filter| filter.matches(path))
        })
    });
    let placements = if wanted {
        Placements::read(archive, options.cancel)?
    } else {
        Placements::default()
    };
    let picked = Scan::run(
        forge,
        files,
        Arc::clone(entities),
        options,
        texts,
        &crafting,
        &placements,
    )?;
    if options.cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    Ok((Collected::new(picked, crafting), placements))
}

fn completeness_of<'o>(options: &'o CatalogOptions<'_>) -> KindCompleteness<'o> {
    options.filter.map_or(KindCompleteness::Complete, |filter| {
        KindCompleteness::Partial {
            filter: filter.as_str(),
        }
    })
}

pub(crate) fn source<'a>(
    archive: &'a Archive,
    exported_at: SystemTime,
    patches: Option<&'a PublicPatches>,
) -> Source<'a> {
    Source {
        channel: channel_name(archive.path()),
        build: archive.build().map(|build| BuildSource {
            branch: build.branch(),
            version: build.version(),
            change_number: build.change_number(),
            build_id: build.build_id(),
            public_patch: patches.and_then(|patches| patches.of(build)),
        }),
        archive_size: archive.file_len(),
        archive_modified: archive.file_modified().map(utc_rfc3339),
        tool_version: env!("CARGO_PKG_VERSION"),
        exported_at: utc_rfc3339(exported_at),
    }
}
