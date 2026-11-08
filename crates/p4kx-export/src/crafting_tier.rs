// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, is_placeholder_key};

use crate::blueprint::{as_struct, craft_time, whole};
use crate::commodity_backfill_input::CommodityInputCapture;
use crate::crafting_types::{
    Cost, CostContext, ItemCost, ModifierKind, PropertyModifier, ResourceCost, Selection, Tier,
    ValueRange,
};
use crate::record_fields::{
    elements, integer, locale_key, number, reference, single, structure, text,
};

const MAX_DEPTH: usize = 32;
const SELECT: &str = "CraftingCost_Select";
const RESOURCE: &str = "CraftingCost_Resource";
const ITEM: &str = "CraftingCost_Item";
const MULTIPLIER: &str = "CraftingCostContext_QuantityMultiplier";
const INCLUSION: &str = "CraftingCostContext_ResultCompositionInclusion";
const MODIFIERS: &str = "CraftingCostContext_ResultGameplayPropertyModifiers";
const LINEAR: &str = "CraftingGameplayPropertyModifierValueRange_Linear";
const LINEAR_ADDITIVE: &str = "CraftingGameplayPropertyModifierValueRange_LinearIntegerAdditive";

/// A `CraftingCost_Select` nested deeper than the reader follows.
#[derive(Debug, thiserror::Error)]
#[error("a selection is nested deeper than {MAX_DEPTH} levels")]
pub(crate) struct TooDeep;

/// Returns the tiers of the `CraftingBlueprint` `blueprint` in stored order. The struct type of
/// every modifier range the reader does not know is pushed to `unknown`.
///
/// # Errors
///
/// Returns [`TooDeep`] when a selection is nested deeper than [`MAX_DEPTH`] levels.
pub(crate) fn tiers<'f>(
    blueprint: Option<&StructValue<'f>>,
    unknown: &mut Vec<&'f str>,
    capture: &mut CommodityInputCapture,
) -> Result<Vec<Tier<'f>>, TooDeep> {
    elements(blueprint, "tiers")
        .iter()
        .enumerate()
        .filter_map(|(index, value)| as_struct(value).map(|item| (index, item)))
        .map(|(index, item)| tier(item, &format!("blueprint.tiers[{index}]"), unknown, capture))
        .collect()
}

fn tier<'f>(
    tier: &StructValue<'f>,
    path: &str,
    unknown: &mut Vec<&'f str>,
    capture: &mut CommodityInputCapture,
) -> Result<Tier<'f>, TooDeep> {
    let costs = structure(structure(Some(tier), "recipe"), "costs");
    let base = format!("{path}.recipe.costs");
    let mut selectors = vec![
        "blueprint".to_owned(),
        "blueprint.$type".to_owned(),
        "blueprint.tiers".to_owned(),
        format!("{path}.$type"),
        format!("{path}.recipe"),
        format!("{path}.recipe.$type"),
        base.clone(),
        format!("{base}.$type"),
    ];
    let mandatory = match structure(costs, "mandatoryCost") {
        Some(cost) => cost_of(
            cost,
            &format!("{base}.mandatoryCost"),
            0,
            unknown,
            capture,
            &mut selectors,
        )?,
        None => None,
    };
    selectors.push(format!("{base}.optionalCosts"));
    let optional = elements(costs, "optionalCosts")
        .iter()
        .enumerate()
        .filter_map(|(index, value)| as_struct(value).map(|cost| (index, cost)))
        .filter_map(|(index, cost)| {
            cost_of(
                cost,
                &format!("{base}.optionalCosts[{index}]"),
                0,
                unknown,
                capture,
                &mut selectors,
            )
            .transpose()
        })
        .collect::<Result<_, _>>()?;
    Ok(Tier {
        craft_time_seconds: craft_time(structure(costs, "craftTime")),
        mandatory,
        optional,
    })
}

fn cost_of<'f>(
    cost: &StructValue<'f>,
    path: &str,
    depth: usize,
    unknown: &mut Vec<&'f str>,
    capture: &mut CommodityInputCapture,
    selectors: &mut Vec<String>,
) -> Result<Option<Cost<'f>>, TooDeep> {
    let kind = cost.type_name();
    if kind == SELECT && depth >= MAX_DEPTH {
        return Err(TooDeep);
    }
    selectors.push(path.to_owned());
    selectors.push(format!("{path}.$type"));
    let result = cost_value(cost, path, depth, unknown, capture, selectors);
    selectors.pop();
    selectors.pop();
    result
}

fn cost_value<'f>(
    cost: &StructValue<'f>,
    path: &str,
    depth: usize,
    unknown: &mut Vec<&'f str>,
    capture: &mut CommodityInputCapture,
    selectors: &mut Vec<String>,
) -> Result<Option<Cost<'f>>, TooDeep> {
    let kind = cost.type_name();
    let context = context(cost, unknown);
    Ok(match kind {
        SELECT => {
            let info = structure(Some(cost), "nameInfo");
            Some(Cost::Select(Selection {
                count: integer(Some(cost), "count"),
                slot: text(info, "debugName"),
                slot_key: locale_key(info, "displayName").filter(|key| !is_placeholder_key(key)),
                context,
                options: selection_options(cost, path, depth, unknown, capture, selectors)?,
            }))
        }
        RESOURCE => {
            let resource = reference(Some(cost), "resource").map(|guid| guid.0);
            if let Some(resource) = resource {
                capture.observe(resource, None, &format!("{path}.resource"), selectors);
            }
            Some(Cost::Resource(ResourceCost {
                resource,
                quantity_scu: single(structure(Some(cost), "quantity"), "standardCargoUnits"),
                min_quality: integer(Some(cost), "minQuality"),
                context,
            }))
        }
        ITEM => Some(Cost::Item(ItemCost {
            item: reference(Some(cost), "entityClass").map(|guid| guid.0),
            quantity: integer(Some(cost), "quantity"),
            min_quality: integer(Some(cost), "minQuality"),
            context,
        })),
        _ => None,
    })
}

fn selection_options<'f>(
    cost: &StructValue<'f>,
    path: &str,
    depth: usize,
    unknown: &mut Vec<&'f str>,
    capture: &mut CommodityInputCapture,
    selectors: &mut Vec<String>,
) -> Result<Vec<Cost<'f>>, TooDeep> {
    selectors.push(format!("{path}.options"));
    let options = elements(Some(cost), "options")
        .iter()
        .enumerate()
        .filter_map(|(index, value)| as_struct(value).map(|option| (index, option)))
        .filter_map(|(index, option)| {
            cost_of(
                option,
                &format!("{path}.options[{index}]"),
                depth.saturating_add(1),
                unknown,
                capture,
                selectors,
            )
            .transpose()
        })
        .collect();
    selectors.pop();
    options
}
fn context<'f>(cost: &StructValue<'f>, unknown: &mut Vec<&'f str>) -> CostContext<'f> {
    let mut context = CostContext::default();
    for part in elements(Some(cost), "context").iter().filter_map(as_struct) {
        match part.type_name() {
            MULTIPLIER => context.quantity_multiplier = number(Some(part), "multiplier"),
            INCLUSION => context.composition_inclusion = text(Some(part), "option"),
            MODIFIERS => context.modifiers.extend(modifiers(part, unknown)),
            _ => {}
        }
    }
    context
}

fn modifiers<'f>(part: &StructValue<'f>, unknown: &mut Vec<&'f str>) -> Vec<PropertyModifier> {
    let list = structure(Some(part), "gameplayPropertyModifiers");
    elements(list, "gameplayPropertyModifiers")
        .iter()
        .filter_map(as_struct)
        .map(|modifier| PropertyModifier {
            property: reference(Some(modifier), "gameplayPropertyRecord").map(|guid| guid.0),
            ranges: elements(Some(modifier), "valueRanges")
                .iter()
                .filter_map(as_struct)
                .filter_map(|item| range(item, unknown))
                .collect(),
        })
        .collect()
}

fn range<'f>(range: &StructValue<'f>, unknown: &mut Vec<&'f str>) -> Option<ValueRange> {
    let (kind, start, end) = match range.type_name() {
        LINEAR => (ModifierKind::Multiplier, "modifierAtStart", "modifierAtEnd"),
        LINEAR_ADDITIVE => (
            ModifierKind::Additive,
            "additiveModifierAtStart",
            "additiveModifierAtEnd",
        ),
        other => {
            unknown.push(other);
            return None;
        }
    };
    Some(ValueRange {
        kind,
        start_quality: whole(Some(range), "startQuality"),
        end_quality: whole(Some(range), "endQuality"),
        at_start: number(Some(range), start),
        at_end: number(Some(range), end),
    })
}
