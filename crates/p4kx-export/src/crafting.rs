// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeSet, HashMap};

use p4kx_extraction::{CancelToken, DataForge, Filter, Guid};

use crate::crafting_types::{
    BlueprintPool, BlueprintTiers, DismantleProcess, DismantleReturn, GameplayProperty,
    GlobalParams, QualityDistribution, QualityQuantization, RewardChance, RewardSource,
};
use crate::crafting_warning::CraftingWarning;
use crate::selection::is_leftover;
use crate::{CatalogError, CatalogFailure, CatalogProgress};

#[path = "crafting_ledger/mod.rs"]
pub(crate) mod ledger;

pub(crate) use ledger::ReadCrafting;

/// The crafting data of a DataForge: the blueprints with all their tiers and what surrounds them.
///
/// Every list is ordered by the GUID of its Record; [`reward_sources`](Self::reward_sources) by
/// the order of the Records that hold them. The blueprints are the same set as the `blueprints`
/// kind of the Basetool catalog (development leftovers left out), selected by the one rule
/// behind both.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Crafting<'f> {
    /// The blueprints under `crafting/blueprints/crafting` with every tier.
    pub blueprints: Vec<BlueprintTiers<'f>>,
    /// The gameplay property definitions the blueprint modifiers refer to.
    pub properties: Vec<GameplayProperty<'f>>,
    /// The quality distributions and their location overrides.
    pub quality_distributions: Vec<QualityDistribution<'f>>,
    /// The quality quantizations, each with the `ResourceType`s whose crafting data link to it.
    pub quality_quantizations: Vec<QualityQuantization<'f>>,
    /// The links of `ResourceType` Records (leftovers excluded) to a Record that is no readable
    /// quality quantization: a GUID of another type or of no Record, or a Record that failed.
    pub unresolved_quantization_links: usize,
    /// The blueprint pools.
    pub pools: Vec<BlueprintPool<'f>>,
    /// The places that reward a blueprint of a pool, found through their Reference to it.
    pub reward_sources: Vec<RewardSource<'f>>,
    /// The generic dismantle processes under `crafting/blueprints/dismantle`.
    pub dismantle: Vec<DismantleProcess<'f>>,
    /// The `CraftingGlobalParams` Records, merged.
    pub global: GlobalParams,
    /// The blueprints left out as development leftovers.
    pub skipped_blueprints: usize,
    /// The Records that could not be read; they are left out.
    pub failures: Vec<CatalogFailure>,
    /// What makes the data incomplete or ambiguous although every Record was read.
    pub warnings: Vec<CraftingWarning>,
}

/// Reads the crafting data of `forge`.
///
/// The Records are read in parallel. A `Filter` keeps only the blueprints and pools whose record
/// path it matches (the reward sources are then those of the kept pools); the properties,
/// quality Records, the `ResourceType` links to the quality quantizations, dismantle processes and
/// global parameters are always read. A Record that cannot be read is left out and listed in [`Crafting::failures`]; a Record outside the `Filter`
/// is not listed, except the global parameters. A Record that could hold a reward source and
/// cannot be read leaves a [`CraftingWarning`].
///
/// # Errors
///
/// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled.
pub fn read_crafting<'f>(
    forge: &'f DataForge<'_>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<Crafting<'f>, CatalogError> {
    ledger::read(forge, filter, cancel, None).map(|read| read.data)
}

/// Reads the selected catalog's crafting inputs and reports its reward-source scan to `progress`.
pub(crate) fn read_with_progress<'f>(
    forge: &'f DataForge<'_>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    progress: Option<&(dyn Fn(CatalogProgress) + Sync)>,
) -> Result<ReadCrafting<'f>, CatalogError> {
    ledger::read_catalog(forge, filter, cancel, progress)
}

impl<'f> Crafting<'f> {
    /// Returns, per blueprint, the chances that a reward source gives it: the chance of the
    /// source times the weight of the blueprint over the sum of the weights of the pool, one
    /// entry per source and pool entry.
    ///
    /// A source without a chance or with one outside 0 to 1, a pool without positive weight and
    /// an entry without a blueprint or with a weight of zero or less give none.
    #[must_use]
    pub fn reward_chances(&self) -> HashMap<Guid, Vec<RewardChance<'f>>> {
        let pools: HashMap<Guid, (&BlueprintPool<'f>, f32)> = self
            .pools
            .iter()
            .map(|pool| (pool.guid, (pool, pool.total_weight())))
            .collect();
        let mut chances: HashMap<Guid, Vec<RewardChance<'f>>> = HashMap::new();
        for source in &self.reward_sources {
            let pool_guid = source.pool;
            let Some(chance) = source.chance else {
                continue;
            };
            if !(0.0..=1.0).contains(&chance) {
                continue;
            }
            let Some((pool, total)) = pools.get(&pool_guid) else {
                continue;
            };
            if !(*total > 0.0 && total.is_finite()) {
                continue;
            }
            for entry in &pool.entries {
                let (Some(blueprint), Some(weight)) = (entry.blueprint, entry.weight) else {
                    continue;
                };
                if weight <= 0.0 {
                    continue;
                }
                chances.entry(blueprint).or_default().push(RewardChance {
                    record: source.record,
                    record_path: source.record_path,
                    contract_id: source.contract_id,
                    contract_name: source.contract_name,
                    pool: pool_guid,
                    chance: f64::from(chance) * f64::from(weight) / f64::from(*total),
                });
            }
        }
        chances
    }

    fn unknown_ranges_warning(&self) -> Option<CraftingWarning> {
        let count = self
            .blueprints
            .iter()
            .map(|blueprint| blueprint.unknown_range_types.len())
            .sum();
        if count == 0 {
            return None;
        }
        let types: BTreeSet<&str> = self
            .blueprints
            .iter()
            .flat_map(|blueprint| blueprint.unknown_range_types.iter().copied())
            .collect();
        Some(CraftingWarning::UnknownModifierRanges {
            count,
            details: types.into_iter().map(str::to_owned).collect(),
        })
    }

    fn live_dismantle(&self) -> impl Iterator<Item = &DismantleProcess<'f>> {
        self.dismantle
            .iter()
            .filter(|process| !is_leftover(process.record_path, process.class_name))
    }

    /// Returns the dismantle process when the DataForge holds exactly one that is no development
    /// leftover, `None` for none or several (a [`CraftingWarning::AmbiguousDismantle`] then
    /// tells).
    #[must_use]
    pub fn dismantle_process(&self) -> Option<&DismantleProcess<'f>> {
        let mut live = self.live_dismantle();
        let first = live.next()?;
        live.next().is_none().then_some(first)
    }

    /// Returns the dismantle return of the item `blueprint` produces: the only dismantle
    /// process, linked to the blueprint whose first tier the item returns by the process's
    /// efficiency, without the resources the global parameters exclude.
    ///
    /// `None` for a blueprint without a produced item, for an item on the dismantle blacklist,
    /// and unless exactly one dismantle process exists. [`DismantleReturn`] states which tier
    /// counts and how a selection and the quantity multiplier are treated.
    #[must_use]
    pub fn dismantle_return<'c>(
        &'c self,
        blueprint: &BlueprintTiers<'_>,
    ) -> Option<DismantleReturn<'c, 'f>> {
        let item = blueprint.produced_item?;
        if self.global.dismantle_blacklist_items.contains(&item) {
            return None;
        }
        Some(DismantleReturn {
            process: self.dismantle_process()?,
            blueprint: blueprint.guid,
            item,
            tier: (!blueprint.tiers.is_empty()).then_some(0),
            excluded_resources: &self.global.dismantle_blacklist_resources,
        })
    }
}
