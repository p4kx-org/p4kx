// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::ReducedError;
use super::completeness;
use crate::CatalogError;
use crate::blueprint_tier_contract::{BlueprintCostMeaning, BlueprintCostPosition, BlueprintTier};
use crate::catalog_query_session::checkpoint;
use crate::source_value_catalog::SourceValueRef;

pub(super) const PATH: &str = "configurations.blueprintTiers";

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BlueprintKey {
    pub(super) record_index: u32,
}

#[derive(Default)]
pub(super) struct RecipeCoordinates<'read> {
    tiers: HashMap<u64, u32>,
    costs: HashMap<u64, IndexedCost<'read>>,
}

struct IndexedCost<'read> {
    tier_index: u32,
    position: &'read BlueprintCostPosition,
    role: Role,
}

#[derive(PartialEq)]
enum Role {
    Selection,
    Ingredient,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CostPosition {
    pub(super) tier_index: u32,
    root: Root,
    options: Vec<u32>,
}

#[derive(PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Root {
    Mandatory,
    Optional { index: u32 },
}

impl<'read> RecipeCoordinates<'read> {
    pub(super) fn new(
        tiers: &'read [BlueprintTier],
        cancel: Option<&CancelToken>,
    ) -> Result<Self, ReducedError> {
        let mut recipes = Self::default();
        recipes
            .tiers
            .try_reserve(tiers.len())
            .map_err(CatalogError::SourceUseAllocation)?;
        let mut previous = None;
        for tier in tiers {
            checkpoint(cancel)?;
            let index = *completeness::cell(PATH, tier.tier_index())?
                .value
                .ok_or_else(invalid)?;
            if previous.is_some_and(|previous| index <= previous)
                || recipes
                    .tiers
                    .insert(tier.source().value_index(), index)
                    .is_some()
            {
                return Err(invalid());
            }
            previous = Some(index);
            recipes.add_costs(tier, index, cancel)?;
        }
        Ok(recipes)
    }

    pub(super) fn tier(&self, source: SourceValueRef) -> Result<u32, ReducedError> {
        self.tiers
            .get(&source.value_index())
            .copied()
            .ok_or_else(invalid)
    }

    pub(super) fn cost(
        &self,
        source: SourceValueRef,
        tier_index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<CostPosition, ReducedError> {
        self.resolve(self.find_in_tier(source, tier_index)?, cancel)
    }

    pub(super) fn ingredient(
        &self,
        source: SourceValueRef,
        slot: Option<SourceValueRef>,
        tier_index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<(CostPosition, Option<CostPosition>), ReducedError> {
        let cost = self.find_in_tier(source, tier_index)?;
        if cost.role != Role::Ingredient {
            return Err(invalid());
        }
        let position = self.resolve(cost, cancel)?;
        let slot = slot
            .map(|source| {
                let selection = self.find_in_tier(source, tier_index)?;
                if selection.role != Role::Selection {
                    return Err(invalid());
                }
                let slot = self.resolve(selection, cancel)?;
                if !position.descends_from(&slot) {
                    return Err(invalid());
                }
                Ok(slot)
            })
            .transpose()?;
        Ok((position, slot))
    }

    fn resolve(
        &self,
        cost: &IndexedCost<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<CostPosition, ReducedError> {
        let mut options = Vec::new();
        let mut position = cost.position;
        let root = loop {
            checkpoint(cancel)?;
            match position {
                BlueprintCostPosition::Mandatory => break Root::Mandatory,
                BlueprintCostPosition::Optional { index } => {
                    break Root::Optional { index: *index };
                }
                BlueprintCostPosition::Option { parent, index } => {
                    position = self.find_in_tier(*parent, cost.tier_index)?.position;
                    options
                        .try_reserve(1)
                        .map_err(CatalogError::SourceUseAllocation)?;
                    options.push(*index);
                }
            }
        };
        options.reverse();
        Ok(CostPosition {
            tier_index: cost.tier_index,
            root,
            options,
        })
    }

    fn find_in_tier(
        &self,
        source: SourceValueRef,
        tier_index: u32,
    ) -> Result<&IndexedCost<'_>, ReducedError> {
        self.costs
            .get(&source.value_index())
            .filter(|cost| cost.tier_index == tier_index)
            .ok_or_else(invalid)
    }

    fn add_costs(
        &mut self,
        tier: &'read BlueprintTier,
        tier_index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<(), ReducedError> {
        self.costs
            .try_reserve(tier.costs().len())
            .map_err(CatalogError::SourceUseAllocation)?;
        for cost in tier.costs() {
            checkpoint(cancel)?;
            let position = completeness::cell(PATH, cost.position())?
                .value
                .ok_or_else(invalid)?;
            if let BlueprintCostPosition::Option { parent, .. } = position
                && self.find_in_tier(*parent, tier_index)?.role != Role::Selection
            {
                return Err(invalid());
            }
            let role = match cost.meaning() {
                BlueprintCostMeaning::Select { .. } => Role::Selection,
                BlueprintCostMeaning::Resource { .. } | BlueprintCostMeaning::Item { .. } => {
                    Role::Ingredient
                }
            };
            if self
                .costs
                .insert(
                    cost.source().value_index(),
                    IndexedCost {
                        tier_index,
                        position,
                        role,
                    },
                )
                .is_some()
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

impl CostPosition {
    fn descends_from(&self, other: &Self) -> bool {
        self.tier_index == other.tier_index
            && self.root == other.root
            && self.options.len() > other.options.len()
            && self.options.starts_with(&other.options)
    }
}

const fn invalid() -> ReducedError {
    ReducedError::IncompleteScope(PATH)
}
