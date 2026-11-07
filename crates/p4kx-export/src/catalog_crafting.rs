// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{Filter, Guid};

use crate::blueprint::{Links, blueprint};
use crate::contract::{
    Blueprint, BlueprintPool, CraftingGlobal, DismantleProcess, GameplayProperty,
    QualityDistribution, QualityQuantization,
};
use crate::contract_quantization::QuantizationBand;
use crate::contract_recipe::BlueprintDismantle;
use crate::contract_rewards::{LocationOverride, PoolEntry, RewardSource};
use crate::crafting::{Crafting, ReadCrafting};
use crate::crafting_types as library;
use crate::crafting_warning::CraftingWarning;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::selection::is_leftover;
use crate::texts::Texts;
use crate::{CatalogFailure, Kind, QuantizationSummary};

#[path = "crafting_ledger/catalog_rows.rs"]
mod rows;

/// The kinds of the catalog that [`Crafting`] fills, each ordered by the text of the GUID.
pub(crate) struct CraftingKinds<'f> {
    pub(crate) blueprints: Vec<Blueprint<'f>>,
    pub(crate) properties: Vec<GameplayProperty<'f>>,
    pub(crate) distributions: Vec<QualityDistribution<'f>>,
    pub(crate) quantizations: Vec<QualityQuantization<'f>>,
    pub(crate) quantization_summary: QuantizationSummary,
    pub(crate) pools: Vec<BlueprintPool<'f>>,
    pub(crate) dismantle: Vec<DismantleProcess<'f>>,
    pub(crate) global: CraftingGlobal,
    pub(crate) skipped: Vec<(Kind, usize)>,
    pub(crate) failures: Vec<CatalogFailure>,
    pub(crate) warnings: Vec<CraftingWarning>,
    pub(crate) ledger: ExportLedger,
}

/// Maps `crafting` to the catalog kinds, its texts resolved by `texts`.
///
/// The blueprints and pools `crafting` read under the Filter are kept as they are; the
/// properties, quality distributions and dismantle processes, which it reads whatever the Filter,
/// are kept only when `filter` matches their record path. Development leftovers of these kinds
/// are left out and counted, like those of every other kind.
pub(crate) fn kinds<'f>(
    read: ReadCrafting<'f>,
    texts: &Texts<'f>,
    filter: Option<&Filter>,
) -> CraftingKinds<'f> {
    let ReadCrafting {
        data: crafting,
        mut ledger,
    } = read;
    let mut selected = rows::Selection::new(filter, &mut ledger);
    let (properties, properties_skipped) = properties(&crafting, texts, &mut selected);
    let (distributions, distributions_skipped) = selected.entities(
        Kind::QualityDistributions,
        &crafting.quality_distributions,
        |item| (item.record_path, item.class_name, CatalogGuid(item.guid)),
        distribution,
        |entity| entity.guid,
    );
    let (quantizations, quantizations_skipped, quantization_summary) =
        quantizations(&crafting, &mut selected);
    let sources = sources_by_pool(&crafting.reward_sources);
    let (pools, pools_skipped) = selected.entities(
        Kind::BlueprintPools,
        &crafting.pools,
        |item| (item.record_path, item.class_name, CatalogGuid(item.guid)),
        |source| pool(source, &sources),
        |entity| entity.guid,
    );
    let (dismantle, dismantle_skipped) = dismantle(&crafting, &mut selected);
    let kept: HashSet<Guid> = dismantle.iter().map(|process| process.guid.0).collect();
    CraftingKinds {
        blueprints: blueprints(&crafting, texts, &kept),
        properties,
        distributions,
        quantizations,
        quantization_summary,
        pools,
        dismantle,
        global: global(&crafting.global),
        skipped: vec![
            (Kind::Blueprints, crafting.skipped_blueprints),
            (Kind::GameplayProperties, properties_skipped),
            (Kind::QualityDistributions, distributions_skipped),
            (Kind::QualityQuantizations, quantizations_skipped),
            (Kind::BlueprintPools, pools_skipped),
            (Kind::DismantleProcesses, dismantle_skipped),
        ],
        failures: crafting.failures,
        warnings: crafting.warnings,
        ledger,
    }
}

fn properties<'f>(
    crafting: &Crafting<'f>,
    texts: &Texts<'f>,
    selected: &mut rows::Selection<'_>,
) -> (Vec<GameplayProperty<'f>>, usize) {
    selected.entities(
        Kind::GameplayProperties,
        &crafting.properties,
        |item| (item.record_path, item.class_name, CatalogGuid(item.guid)),
        |source| GameplayProperty {
            guid: CatalogGuid(source.guid),
            class_name: source.class_name,
            record_path: source.record_path,
            name: texts.localised(source.name_key),
        },
        |entity| entity.guid,
    )
}

fn dismantle<'f>(
    crafting: &Crafting<'f>,
    selected: &mut rows::Selection<'_>,
) -> (Vec<DismantleProcess<'f>>, usize) {
    selected.entities(
        Kind::DismantleProcesses,
        &crafting.dismantle,
        |item| (item.record_path, item.class_name, CatalogGuid(item.guid)),
        |source| DismantleProcess {
            guid: CatalogGuid(source.guid),
            class_name: source.class_name,
            record_path: source.record_path,
            efficiency: source.efficiency,
            time_seconds: source.time_seconds,
        },
        |entity| entity.guid,
    )
}

/// Returns the quality quantizations the Filter keeps, the number of leftovers, and the counts of
/// the Records no `ResourceType` uses and of the links that reach no kept Record.
fn quantizations<'f>(
    crafting: &Crafting<'f>,
    selected: &mut rows::Selection<'_>,
) -> (Vec<QualityQuantization<'f>>, usize, QuantizationSummary) {
    let (kept, skipped) = selected.entities(
        Kind::QualityQuantizations,
        &crafting.quality_quantizations,
        |item| (item.record_path, item.class_name, CatalogGuid(item.guid)),
        quantization,
        |entity| entity.guid,
    );
    let leftover_links: usize = crafting
        .quality_quantizations
        .iter()
        .filter(|item| selected.matches(item.record_path))
        .filter(|item| is_leftover(item.record_path, item.class_name))
        .map(|item| item.resource_types.len())
        .sum();
    let summary = QuantizationSummary {
        unused: kept
            .iter()
            .filter(|entity| entity.resource_type_guids.is_empty())
            .count(),
        unresolved_links: crafting
            .unresolved_quantization_links
            .saturating_add(leftover_links),
    };
    (kept, skipped, summary)
}

/// Returns the blueprints of `crafting`; one links a dismantle process only when that process is
/// among the `kept` ones, so the link never points at a process the catalog leaves out.
fn blueprints<'f>(
    crafting: &Crafting<'f>,
    texts: &Texts<'f>,
    kept: &HashSet<Guid>,
) -> Vec<Blueprint<'f>> {
    let defaults: HashSet<Guid> = crafting.global.default_blueprints.iter().copied().collect();
    let mut blueprints: Vec<Blueprint<'f>> = crafting
        .blueprints
        .iter()
        .map(|source| {
            let links = Links {
                default_available: defaults.contains(&source.guid),
                dismantle: crafting
                    .dismantle_return(source)
                    .filter(|link| kept.contains(&link.process.guid))
                    .map(|link| BlueprintDismantle {
                        process_guid: CatalogGuid(link.process.guid),
                        tier_index: link.tier.and_then(|tier| u32::try_from(tier).ok()),
                    }),
            };
            blueprint(source, texts, links)
        })
        .collect();
    blueprints.sort_by_cached_key(|entity| entity.guid.text());
    blueprints
}

fn global(source: &library::GlobalParams) -> CraftingGlobal {
    let sorted = |guids: &[Guid]| {
        let mut guids: Vec<CatalogGuid> = guids.iter().copied().map(CatalogGuid).collect();
        guids.sort_by_key(|guid| guid.text());
        guids.dedup_by_key(|guid| guid.text());
        guids
    };
    CraftingGlobal {
        dismantle_excluded_resource_guids: sorted(&source.dismantle_blacklist_resources),
        dismantle_excluded_item_guids: sorted(&source.dismantle_blacklist_items),
    }
}

fn sources_by_pool<'a, 'f>(
    reward_sources: &'a [library::RewardSource<'f>],
) -> HashMap<Guid, Vec<&'a library::RewardSource<'f>>> {
    let mut sources: HashMap<Guid, Vec<&library::RewardSource<'f>>> = HashMap::new();
    for source in reward_sources {
        sources.entry(source.pool).or_default().push(source);
    }
    sources
}

fn pool<'f>(
    source: &library::BlueprintPool<'f>,
    sources: &HashMap<Guid, Vec<&library::RewardSource<'f>>>,
) -> BlueprintPool<'f> {
    let mut reward_sources: Vec<RewardSource<'f>> = sources
        .get(&source.guid)
        .into_iter()
        .flatten()
        .map(|reward| RewardSource {
            record_guid: CatalogGuid(reward.record),
            record_path: reward.record_path,
            contract_id: reward.contract_id.map(CatalogGuid),
            contract_name: reward.contract_name,
            chance: reward.chance,
        })
        .collect();
    reward_sources.sort_by_cached_key(|reward| reward.record_guid.text());
    BlueprintPool {
        guid: CatalogGuid(source.guid),
        class_name: source.class_name,
        record_path: source.record_path,
        total_weight: source.total_weight(),
        entries: source
            .entries
            .iter()
            .map(|entry| PoolEntry {
                blueprint_guid: entry.blueprint.map(CatalogGuid),
                weight: entry.weight,
            })
            .collect(),
        reward_sources,
    }
}

fn distribution<'f>(source: &library::QualityDistribution<'f>) -> QualityDistribution<'f> {
    QualityDistribution {
        guid: CatalogGuid(source.guid),
        class_name: source.class_name,
        record_path: source.record_path,
        distribution: source.distribution,
        overrides: source
            .overrides
            .iter()
            .map(|over| LocationOverride {
                location_guid: over.location.map(CatalogGuid),
                distribution: over.distribution,
            })
            .collect(),
    }
}

fn quantization<'f>(source: &library::QualityQuantization<'f>) -> QualityQuantization<'f> {
    let mut resource_type_guids: Vec<CatalogGuid> = source
        .resource_types
        .iter()
        .copied()
        .map(CatalogGuid)
        .collect();
    resource_type_guids.sort_by_cached_key(|guid| guid.text());
    QualityQuantization {
        guid: CatalogGuid(source.guid),
        class_name: source.class_name,
        record_path: source.record_path,
        bands: source
            .bands
            .iter()
            .map(|band| QuantizationBand {
                start: band.start,
                end: band.end,
                mapped_value: band.mapped_value,
            })
            .collect(),
        resource_type_guids,
    }
}
