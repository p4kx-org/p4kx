// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, is_placeholder_key};

use crate::blueprint;
use crate::crafting_types::{
    Cost, CostContext, ItemCost, ModifierKind, PropertyModifier, ResourceCost, Selection,
    ValueRange,
};
use crate::record_fields::{
    elements, integer, locale_key, number, reference, single, structure, text,
};

const SELECT: &str = "CraftingCost_Select";
const RESOURCE: &str = "CraftingCost_Resource";
const ITEM: &str = "CraftingCost_Item";
const MULTIPLIER: &str = "CraftingCostContext_QuantityMultiplier";
const INCLUSION: &str = "CraftingCostContext_ResultCompositionInclusion";
const MODIFIERS: &str = "CraftingCostContext_ResultGameplayPropertyModifiers";
const LINEAR: &str = "CraftingGameplayPropertyModifierValueRange_Linear";
const LINEAR_ADDITIVE: &str = "CraftingGameplayPropertyModifierValueRange_LinearIntegerAdditive";

pub(super) fn read<'source>(
    cost: &StructValue<'source>,
    unknown: &mut Vec<&'source str>,
) -> Option<Cost<'source>> {
    let context = context(cost, unknown);
    match cost.type_name() {
        SELECT => {
            let info = structure(Some(cost), "nameInfo");
            Some(Cost::Select(Selection {
                count: integer(Some(cost), "count"),
                slot: text(info, "debugName"),
                slot_key: locale_key(info, "displayName").filter(|key| !is_placeholder_key(key)),
                context,
                options: Vec::new(),
            }))
        }
        RESOURCE => Some(Cost::Resource(ResourceCost {
            resource: reference(Some(cost), "resource").map(|guid| guid.0),
            quantity_scu: single(structure(Some(cost), "quantity"), "standardCargoUnits"),
            min_quality: integer(Some(cost), "minQuality"),
            context,
        })),
        ITEM => Some(Cost::Item(ItemCost {
            item: reference(Some(cost), "entityClass").map(|guid| guid.0),
            quantity: integer(Some(cost), "quantity"),
            min_quality: integer(Some(cost), "minQuality"),
            context,
        })),
        _ => None,
    }
}

pub(super) enum ContextPart {
    QuantityMultiplier,
    CompositionInclusion,
    PropertyModifiers,
}

pub(super) fn context_part(part: &StructValue<'_>) -> Option<ContextPart> {
    match part.type_name() {
        MULTIPLIER => Some(ContextPart::QuantityMultiplier),
        INCLUSION => Some(ContextPart::CompositionInclusion),
        MODIFIERS => Some(ContextPart::PropertyModifiers),
        _ => None,
    }
}

pub(super) fn context_parts<'read, 'source>(
    cost: &'read StructValue<'source>,
) -> impl Iterator<Item = &'read StructValue<'source>> + use<'read, 'source> {
    elements(Some(cost), "context")
        .iter()
        .filter_map(blueprint::as_struct)
}

pub(super) fn modifier_sources<'read, 'source>(
    part: &'read StructValue<'source>,
) -> impl Iterator<Item = &'read StructValue<'source>> + use<'read, 'source> {
    let list = structure(Some(part), "gameplayPropertyModifiers");
    elements(list, "gameplayPropertyModifiers")
        .iter()
        .filter_map(blueprint::as_struct)
}

pub(super) fn range_sources<'read, 'source>(
    modifier: &'read StructValue<'source>,
) -> impl Iterator<Item = &'read StructValue<'source>> + use<'read, 'source> {
    elements(Some(modifier), "valueRanges")
        .iter()
        .filter_map(blueprint::as_struct)
}

fn context<'f>(cost: &StructValue<'f>, unknown: &mut Vec<&'f str>) -> CostContext<'f> {
    let mut context = CostContext::default();
    for part in context_parts(cost) {
        match context_part(part) {
            Some(ContextPart::QuantityMultiplier) => {
                context.quantity_multiplier = number(Some(part), "multiplier");
            }
            Some(ContextPart::CompositionInclusion) => {
                context.composition_inclusion = text(Some(part), "option");
            }
            Some(ContextPart::PropertyModifiers) => {
                context.modifiers.extend(modifiers(part, unknown));
            }
            None => {}
        }
    }
    context
}

fn modifiers<'f>(part: &StructValue<'f>, unknown: &mut Vec<&'f str>) -> Vec<PropertyModifier> {
    modifier_sources(part)
        .map(|modifier| PropertyModifier {
            property: reference(Some(modifier), "gameplayPropertyRecord").map(|guid| guid.0),
            ranges: range_sources(modifier)
                .filter_map(|item| range(item, unknown))
                .collect(),
        })
        .collect()
}

fn range<'f>(range: &StructValue<'f>, unknown: &mut Vec<&'f str>) -> Option<ValueRange> {
    let Some((kind, start, end)) = range_kind(range) else {
        unknown.push(range.type_name());
        return None;
    };
    Some(ValueRange {
        kind,
        start_quality: blueprint::whole(Some(range), "startQuality"),
        end_quality: blueprint::whole(Some(range), "endQuality"),
        at_start: number(Some(range), start),
        at_end: number(Some(range), end),
    })
}

pub(super) fn range_kind(
    source: &StructValue<'_>,
) -> Option<(ModifierKind, &'static str, &'static str)> {
    match source.type_name() {
        LINEAR => Some((ModifierKind::Multiplier, "modifierAtStart", "modifierAtEnd")),
        LINEAR_ADDITIVE => Some((
            ModifierKind::Additive,
            "additiveModifierAtStart",
            "additiveModifierAtEnd",
        )),
        _ => None,
    }
}
