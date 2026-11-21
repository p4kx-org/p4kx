// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;

use crate::catalog_table::CatalogTable;
use crate::catalog_v3::industry_modules::blueprint_pool_definition::{
    BlueprintPoolDefinition, BlueprintPoolWeights, PreparedBlueprintPool,
};
use crate::catalog_v3::industry_modules::blueprint_reward_source::BlueprintRewardSource;
use crate::crafting_records::POOLS;
use crate::guid_text::CatalogGuid;

pub(crate) struct BlueprintPoolTables<'source> {
    pub(crate) definitions: CatalogTable<BlueprintPoolDefinition<'source>>,
    pub(crate) weights: CatalogTable<BlueprintPoolWeights>,
    pub(crate) rewards: CatalogTable<BlueprintRewardSource<'source>>,
}

pub(crate) fn tables<'source>(
    forge: &DataForge<'_>,
    mut pools: Vec<PreparedBlueprintPool<'source>>,
    mut rewards: Vec<BlueprintRewardSource<'source>>,
) -> BlueprintPoolTables<'source> {
    order(&mut pools);
    rewards.sort_by_key(|row| {
        (
            row.source().record_index().value().copied(),
            row.source_ordinal().value().map(|ordinal| ordinal.get()),
        )
    });
    let (definitions, weights) = pools
        .into_iter()
        .map(|row| (row.definition, row.weights))
        .unzip();
    BlueprintPoolTables {
        definitions: CatalogTable::new(forge, &[POOLS.type_name], definitions),
        weights: CatalogTable::new(forge, &[POOLS.type_name], weights),
        rewards: CatalogTable::new(forge, &[POOLS.type_name], rewards),
    }
}

pub(crate) fn order(pools: &mut [PreparedBlueprintPool<'_>]) {
    pools.sort_by_cached_key(|pool| (CatalogGuid(pool.physical_guid).text(), pool.physical_index));
}
