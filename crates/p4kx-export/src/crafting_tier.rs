// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree, StructValue, Value};

use crate::CatalogError;
use crate::blueprint;
use crate::crafting_recipe_costs::RecipeCosts;
use crate::crafting_research;
use crate::crafting_types::Tier;
use crate::record_fields;

/// One interpreted tier and the corresponding original source slots.
pub(crate) struct InterpretedTier<'read, 'source> {
    pub(crate) tier: Tier<'source>,
    pub(crate) original: &'read Value<'source>,
    pub(crate) costs: Vec<&'read Value<'source>>,
}

/// Reads every semantic tier and cost in original order.
pub(crate) fn tiers<'read, 'source>(
    blueprint: Option<&'read StructValue<'source>>,
    tree: &RecordTree<'source>,
    unknown: &mut Vec<&'source str>,
    cancel: Option<&CancelToken>,
) -> Result<Vec<InterpretedTier<'read, 'source>>, CatalogError> {
    let mut tiers = Vec::new();
    for original in record_fields::elements(blueprint, "tiers") {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        if let Value::Struct(source) = original {
            tiers.push(tier(source, original, tree, unknown, cancel)?);
        }
    }
    Ok(tiers)
}

fn tier<'read, 'source>(
    source: &'read StructValue<'source>,
    original: &'read Value<'source>,
    tree: &RecordTree<'source>,
    unknown: &mut Vec<&'source str>,
    cancel: Option<&CancelToken>,
) -> Result<InterpretedTier<'read, 'source>, CatalogError> {
    let recipe = record_fields::structure(Some(source), "recipe");
    let costs = record_fields::structure(recipe, "costs");
    let mut arena = RecipeCosts::default();
    let mandatory = costs
        .and_then(|costs| costs.field("mandatoryCost"))
        .map(|cost| arena.read(cost, unknown, cancel))
        .transpose()?
        .flatten();
    let mut optional = Vec::new();
    for cost in record_fields::elements(costs, "optionalCosts") {
        if let Some(index) = arena.read(cost, unknown, cancel)? {
            optional.push(index);
        }
    }
    let time = blueprint::craft_time(record_fields::structure(costs, "craftTime"));
    Ok(InterpretedTier {
        tier: Tier::new(
            time,
            mandatory,
            optional,
            arena.values,
            crafting_research::read(source, tree),
        ),
        original,
        costs: arena.originals,
    })
}
