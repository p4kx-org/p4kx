// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{Filter, Guid};

use crate::blueprint::{Links, blueprint};
use crate::catalog_v3::gameplay_property_catalog::GameplayPropertyDefinition;
use crate::contract::{Blueprint, BlueprintPool, CraftingGlobal, DismantleProcess};
use crate::contract_recipe::BlueprintDismantle;
use crate::contract_rewards::{PoolEntry, RewardSource};
use crate::crafting::{Crafting, ReadCrafting};
use crate::crafting_types as library;
use crate::crafting_warning::CraftingWarning;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::quality_catalog::QualityRows;
use crate::texts::Texts;
use crate::{CatalogFailure, Kind};

#[path = "crafting_ledger/catalog_rows.rs"]
mod rows;

/// Selected crafting and canonical source datasets in their documented GUID order.
pub(crate) struct CraftingKinds<'f> {
    pub(crate) blueprints: Vec<Blueprint<'f>>,
    pub(crate) properties: Vec<GameplayPropertyDefinition<'f>>,
    pub(crate) quality: QualityRows<'f>,
    pub(crate) pools: Vec<BlueprintPool<'f>>,
    pub(crate) dismantle: Vec<DismantleProcess<'f>>,
    pub(crate) global: CraftingGlobal,
    pub(crate) skipped: Vec<(Kind, usize)>,
    pub(crate) failures: Vec<CatalogFailure>,
    pub(crate) warnings: Vec<CraftingWarning>,
    pub(crate) ledger: ExportLedger,
}

/// Combines canonical gameplay and quality facts with interpreted crafting datasets.
/// Other crafting rows apply their own catalog Filter and development-leftover rules.
pub(crate) fn kinds<'f>(
    read: ReadCrafting<'f>,
    texts: &Texts<'f>,
    filter: Option<&Filter>,
) -> CraftingKinds<'f> {
    let ReadCrafting {
        data: crafting,
        mut ledger,
        gameplay,
        quality,
    } = read;
    let mut selected = rows::Selection::new(filter, &mut ledger);
    let properties = gameplay.definitions;
    let properties_skipped = gameplay.skipped;
    let distributions_skipped = quality.skipped_distributions;
    let quantizations_skipped = quality.skipped_quantizations;
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
        quality,
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
