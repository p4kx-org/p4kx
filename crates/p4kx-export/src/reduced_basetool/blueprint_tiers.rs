// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::blueprint_cost_context::{self, Context};
use super::blueprint_recipe_coordinates::{BlueprintKey, CostPosition, PATH, RecipeCoordinates};
use super::cell::Cell;
use super::completeness;
use crate::CatalogError;
use crate::blueprint_tier_contract::{BlueprintCost, BlueprintCostMeaning, BlueprintTier};
use crate::catalog_query_session::checkpoint;
use crate::catalog_rows::staged::StagedCatalogRows;
use crate::guid_text::CatalogGuid;
use crate::resource_integer::ResourceInt64;
use crate::source_value_catalog::{SourceArenaOutput, SourceText, SourceValueRef};

pub(super) fn append(
    output: &mut StagedCatalogRows<'_>,
    blueprint: BlueprintKey,
    tiers: &[BlueprintTier],
    coordinates: &RecipeCoordinates<'_>,
    original: &SourceArenaOutput<'_>,
    cancel: Option<&CancelToken>,
) -> Result<(), ReducedError> {
    for tier in tiers {
        checkpoint(cancel)?;
        let recipe = Recipe {
            original,
            coordinates,
            tier_index: coordinates.tier(tier.source())?,
            cancel,
        };
        output.append(&Tier::read(blueprint, tier, &recipe)?)?;
    }
    Ok(())
}

struct Recipe<'read, 'source> {
    original: &'read SourceArenaOutput<'source>,
    coordinates: &'read RecipeCoordinates<'read>,
    tier_index: u32,
    cancel: Option<&'read CancelToken>,
}

impl Recipe<'_, '_> {
    fn position(&self, source: SourceValueRef) -> Result<CostPosition, ReducedError> {
        self.coordinates.cost(source, self.tier_index, self.cancel)
    }

    fn positions(&self, sources: &[SourceValueRef]) -> Result<Vec<CostPosition>, ReducedError> {
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(sources.len())
            .map_err(CatalogError::SourceUseAllocation)?;
        for source in sources {
            positions.push(self.position(*source)?);
        }
        Ok(positions)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Tier<'read, 'source> {
    blueprint: BlueprintKey,
    #[serde(rename = "tierIndex")]
    index: Cell<&'read u32>,
    craft_time_seconds: Cell<&'read ResourceInt64>,
    mandatory: Option<CostPosition>,
    optional: Vec<CostPosition>,
    costs: Vec<Cost<'read, 'source>>,
}

impl<'read, 'source> Tier<'read, 'source> {
    fn read(
        blueprint: BlueprintKey,
        source: &'read BlueprintTier,
        recipe: &Recipe<'read, 'source>,
    ) -> Result<Self, ReducedError> {
        let mut costs = Vec::new();
        costs
            .try_reserve_exact(source.costs().len())
            .map_err(CatalogError::SourceUseAllocation)?;
        for cost in source.costs() {
            costs.push(Cost::read(cost, recipe)?);
        }
        Ok(Self {
            blueprint,
            index: completeness::cell(PATH, source.tier_index())?,
            craft_time_seconds: completeness::cell(PATH, source.craft_time_seconds())?,
            mandatory: source
                .mandatory()
                .map(|cost| recipe.position(cost))
                .transpose()?,
            optional: recipe.positions(source.optional())?,
            costs,
        })
    }
}

#[derive(Serialize)]
struct Cost<'read, 'source> {
    position: Cell<CostPosition>,
    context: Context<'read, 'source>,
    meaning: Meaning<'read, 'source>,
}

impl<'read, 'source> Cost<'read, 'source> {
    fn read(
        source: &'read BlueprintCost,
        recipe: &Recipe<'read, 'source>,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            position: completeness::cell(PATH, source.position())?
                .try_map(|_| recipe.position(source.source()))?,
            context: Context::read(source.context(), recipe.original, recipe.cancel)?,
            meaning: Meaning::read(source.meaning(), recipe)?,
        })
    }
}

#[derive(Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Meaning<'read, 'source> {
    Select {
        count: Cell<&'read ResourceInt64>,
        debug_name: Option<Cell<&'read SourceText<'source>>>,
        options: Vec<CostPosition>,
    },
    Resource {
        resource_guid: Cell<&'read CatalogGuid>,
        quantity_scu: Cell<&'read f32>,
        min_quality: Cell<&'read ResourceInt64>,
    },
    Item {
        item_guid: Cell<&'read CatalogGuid>,
        quantity_units: Cell<&'read ResourceInt64>,
        min_quality: Cell<&'read ResourceInt64>,
    },
}

impl<'read, 'source> Meaning<'read, 'source> {
    fn read(
        source: &'read BlueprintCostMeaning,
        recipe: &Recipe<'read, 'source>,
    ) -> Result<Self, ReducedError> {
        Ok(match source {
            BlueprintCostMeaning::Select {
                count,
                debug_name,
                options,
                ..
            } => Self::Select {
                count: field(count)?,
                debug_name: blueprint_cost_context::text(recipe.original, *debug_name)?,
                options: recipe.positions(options)?,
            },
            BlueprintCostMeaning::Resource {
                resource_guid,
                quantity_scu,
                min_quality,
            } => Self::Resource {
                resource_guid: field(resource_guid)?,
                quantity_scu: field(quantity_scu)?,
                min_quality: field(min_quality)?,
            },
            BlueprintCostMeaning::Item {
                item_guid,
                quantity_units,
                min_quality,
            } => Self::Item {
                item_guid: field(item_guid)?,
                quantity_units: field(quantity_units)?,
                min_quality: field(min_quality)?,
            },
        })
    }
}

fn field<T>(source: &SourcedValue<T>) -> Result<Cell<&T>, ReducedError> {
    completeness::cell(PATH, source)
}
