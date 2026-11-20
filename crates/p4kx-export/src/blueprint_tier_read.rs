// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourcedValue, StructValue, Value};

use crate::blueprint_definition::PhysicalBlueprintRef;
use crate::blueprint_field_source::{BlueprintField, normalize, value_ref};
use crate::blueprint_tier_contract::{
    BlueprintCost, BlueprintCostMeaning, BlueprintCostPosition, BlueprintTier,
};
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_text_contract::GameTextRef;
use crate::crafting_tier::InterpretedTier;
use crate::crafting_types::{Cost, CostIndex};
use crate::resource_integer::ResourceInt64;
use crate::source_fields::{FieldCapture, FieldError};
use crate::source_value_catalog::SourceValueRef;
use crate::{CatalogError, ExportAccountingError};

#[path = "blueprint_cost_position.rs"]
mod position;

pub(super) fn read<'source>(
    tier: &InterpretedTier<'_, 'source>,
    owner: PhysicalBlueprintRef,
    projection: &CatalogProjection<'_, 'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    capture: &dyn FieldCapture<'source>,
    notes: &mut Vec<FieldError>,
) -> Result<BlueprintTier, CatalogError> {
    let source = original_struct(tier.original)?;
    let original_tier = fields.fact(tier.original)?;
    let costs = tier
        .tier
        .costs()
        .iter()
        .zip(&tier.costs)
        .map(|(cost, original)| cost_row(cost, original, tier, projection, fields))
        .collect::<Result<Vec<_>, _>>()?;
    let research = crate::blueprint_research_read::read(
        projection.original(),
        fields,
        capture,
        source,
        notes,
    )?;
    Ok(BlueprintTier {
        blueprint: owner,
        tier_index: position::tier_index(original_tier, fields)?,
        source: value_ref(original_tier),
        craft_time_seconds: craft_time(tier, source, fields)?,
        mandatory: tier
            .tier
            .mandatory
            .map(|index| cost_ref(tier, index, fields))
            .transpose()?,
        optional: cost_refs(tier, &tier.tier.optional, fields)?,
        costs,
        research,
    })
}

fn original_struct<'read, 'source>(
    value: &'read Value<'source>,
) -> Result<&'read StructValue<'source>, CatalogError> {
    crate::blueprint::as_struct(value)
        .ok_or_else(|| ExportAccountingError::ConflictingSourceDatum.into())
}

fn cost_row<'source>(
    cost: &Cost<'source>,
    original: &Value<'source>,
    tier: &InterpretedTier<'_, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
) -> Result<BlueprintCost, CatalogError> {
    let source = original_struct(original)?;
    let field = |name| BlueprintField::read(fields, source, name);
    let meaning = match cost {
        Cost::Select(selection) => {
            let name = field("nameInfo")?;
            BlueprintCostMeaning::Select {
                count: integer(&field("count")?, selection.count)?,
                debug_name: name.child("debugName")?.reference(),
                display_name: text(&name.child("displayName")?, projection)?,
                options: cost_refs(tier, &selection.options, fields)?,
            }
        }
        Cost::Resource(resource) => BlueprintCostMeaning::Resource {
            resource_guid: field("resource")?.derived(|value| {
                normalize(value, resource.resource.map(crate::guid_text::CatalogGuid))
            })?,
            quantity_scu: field("quantity")?
                .child("standardCargoUnits")?
                .derived(|value| normalize(value, resource.quantity_scu))?,
            min_quality: integer(&field("minQuality")?, resource.min_quality)?,
        },
        Cost::Item(item) => BlueprintCostMeaning::Item {
            item_guid: field("entityClass")?
                .derived(|value| normalize(value, item.item.map(crate::guid_text::CatalogGuid)))?,
            quantity_units: integer(&field("quantity")?, item.quantity)?,
            min_quality: integer(&field("minQuality")?, item.min_quality)?,
        },
    };
    let original = fields.fact(original)?;
    Ok(BlueprintCost {
        source: value_ref(original),
        position: position::cost_position(original, fields)?,
        context: crate::blueprint_context_read::read(source, fields)?,
        meaning,
    })
}

fn cost_ref(
    tier: &InterpretedTier<'_, '_>,
    index: CostIndex,
    fields: &OriginalFlowFields<'_, '_, '_>,
) -> Result<SourceValueRef, CatalogError> {
    let value = tier
        .costs
        .get(index.index())
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    Ok(value_ref(fields.fact(value)?))
}

fn cost_refs(
    tier: &InterpretedTier<'_, '_>,
    indices: &[CostIndex],
    fields: &OriginalFlowFields<'_, '_, '_>,
) -> Result<Vec<SourceValueRef>, CatalogError> {
    indices
        .iter()
        .map(|index| cost_ref(tier, *index, fields))
        .collect()
}

fn integer(
    field: &BlueprintField<'_, '_, '_, '_>,
    value: Option<i64>,
) -> Result<SourcedValue<ResourceInt64>, CatalogError> {
    field.derived(|source| normalize(source, value.map(ResourceInt64::new)))
}

pub(super) fn text<'source>(
    field: &BlueprintField<'_, '_, '_, 'source>,
    projection: &CatalogProjection<'_, 'source>,
) -> Result<SourcedValue<GameTextRef>, CatalogError> {
    match field.fact() {
        Some(fact) if !field.is_blocked() => projection.description_fact(fact),
        _ => field.derived(|value| normalize(value, None)),
    }
}

fn craft_time<'source>(
    tier: &InterpretedTier<'_, 'source>,
    source: &StructValue<'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
) -> Result<SourcedValue<ResourceInt64>, CatalogError> {
    BlueprintField::read(fields, source, "recipe")?
        .child("costs")?
        .child("craftTime")?
        .duration_seconds(tier.tier.craft_time_seconds)
}
