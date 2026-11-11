// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{Filter, Guid};

use crate::CatalogError;
use crate::Kind;
use crate::blueprint_catalog_rows::{self, BlueprintRow};
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read_failure::CatalogReadFailure;
use crate::catalog_v3::gameplay_property_catalog::GameplayPropertyDefinition;
use crate::contract::{BlueprintPool, CraftingGlobal, DismantleProcess};
use crate::contract_rewards::{PoolEntry, RewardSource};
use crate::crafting::{Crafting, ReadCrafting};
use crate::crafting_types as library;
use crate::crafting_warning::CraftingWarning;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::quality_catalog::QualityRows;

#[path = "crafting_ledger/catalog_rows.rs"]
mod rows;

/// Selected crafting and canonical source datasets in their documented GUID order.
pub(crate) struct CraftingKinds<'f> {
    pub(crate) blueprints: Vec<BlueprintRow<'f>>,
    pub(crate) properties: Vec<GameplayPropertyDefinition<'f>>,
    pub(crate) quality: QualityRows<'f>,
    pub(crate) pools: Vec<BlueprintPool<'f>>,
    pub(crate) dismantle: Vec<DismantleProcess<'f>>,
    pub(crate) global: CraftingGlobal,
    pub(crate) skipped: Vec<(Kind, usize)>,
    pub(crate) failures: Vec<CatalogReadFailure<'f>>,
    pub(crate) warnings: Vec<CraftingWarning>,
    pub(crate) ledger: ExportLedger,
}

/// Combines canonical gameplay and quality facts with interpreted crafting datasets.
/// Other crafting rows apply their own catalog Filter and development-leftover rules.
pub(crate) fn kinds<'f>(
    read: ReadCrafting<'f>,
    origins: &CatalogOrigins,
    filter: Option<&Filter>,
) -> Result<CraftingKinds<'f>, CatalogError> {
    let ReadCrafting {
        data: crafting,
        blueprints,
        blueprint_rules,
        catalog_failures,
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
    let process = crafting
        .dismantle_process()
        .map(|item| item.guid)
        .filter(|guid| kept.contains(guid));
    let mut blueprints = blueprints
        .into_iter()
        .map(|row| blueprint_catalog_rows::row(row, &blueprint_rules, process, origins))
        .collect::<Result<Vec<_>, _>>()?;
    blueprints.sort_by_cached_key(|row| {
        (
            row.definition.physical_guid.text(),
            row.definition.physical_index,
        )
    });
    Ok(CraftingKinds {
        blueprints,
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
        failures: catalog_failures,
        warnings: crafting.warnings,
        ledger,
    })
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
