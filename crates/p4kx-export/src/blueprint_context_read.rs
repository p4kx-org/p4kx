// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourcedValue, StructValue};

use crate::blueprint_context_contract::{
    BlueprintCostContext, BlueprintModifier, BlueprintModifierRange, BlueprintRangeKind,
};
use crate::blueprint_field_source::{BlueprintField, normalize, value_ref};
use crate::catalog_read::original::OriginalFlowFields;
use crate::crafting_cost_read::{self, ContextPart};
use crate::crafting_types::ModifierKind;
use crate::resource_integer::ResourceInt64;
use crate::{CatalogError, ExportAccountingError};

pub(super) fn read<'source>(
    cost: &StructValue<'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
) -> Result<BlueprintCostContext, CatalogError> {
    let mut result = BlueprintCostContext {
        source: BlueprintField::read(fields, cost, "context")?.reference(),
        ..BlueprintCostContext::default()
    };
    for part in crafting_cost_read::context_parts(cost) {
        fields.check_cancel()?;
        match crafting_cost_read::context_part(part) {
            Some(ContextPart::QuantityMultiplier) => {
                result.quantity_multiplier = Some(number(fields, part, "multiplier")?);
            }
            Some(ContextPart::CompositionInclusion) => {
                result.composition_inclusion =
                    BlueprintField::read(fields, part, "option")?.reference();
            }
            Some(ContextPart::PropertyModifiers) => {
                for source in crafting_cost_read::modifier_sources(part) {
                    result.modifiers.push(modifier(source, fields)?);
                }
            }
            None => {}
        }
    }
    Ok(result)
}

fn modifier<'source>(
    source: &StructValue<'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
) -> Result<BlueprintModifier, CatalogError> {
    let original = fields
        .owner_of(source)?
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let property_guid = BlueprintField::read(fields, source, "gameplayPropertyRecord")?
        .derived(|value| normalize(value, crate::record_fields::record_reference(value)))?;
    let mut ranges = Vec::new();
    for range in crafting_cost_read::range_sources(source) {
        fields.check_cancel()?;
        if let Some(projected) = range_row(range, fields)? {
            ranges.push(projected);
        }
    }
    Ok(BlueprintModifier {
        source: value_ref(original),
        property_guid,
        ranges,
    })
}

fn range_row<'source>(
    source: &StructValue<'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
) -> Result<Option<BlueprintModifierRange>, CatalogError> {
    let Some((kind, start, end)) = crafting_cost_read::range_kind(source) else {
        return Ok(None);
    };
    let original = fields
        .owner_of(source)?
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let kind = match kind {
        ModifierKind::Multiplier => BlueprintRangeKind::Multiplier,
        ModifierKind::Additive => BlueprintRangeKind::Additive,
    };
    Ok(Some(BlueprintModifierRange {
        source: value_ref(original),
        kind,
        start_quality: quality(fields, source, "startQuality")?,
        end_quality: quality(fields, source, "endQuality")?,
        at_start: number(fields, source, start)?,
        at_end: number(fields, source, end)?,
    }))
}

fn quality<'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    source: &StructValue<'source>,
    name: &str,
) -> Result<SourcedValue<ResourceInt64>, CatalogError> {
    BlueprintField::read(fields, source, name)?.derived(|value| {
        normalize(
            value,
            crate::blueprint::whole(Some(source), name).map(ResourceInt64::new),
        )
    })
}

fn number<'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    source: &StructValue<'source>,
    name: &str,
) -> Result<SourcedValue<f32>, CatalogError> {
    BlueprintField::read(fields, source, name)?
        .derived(|value| normalize(value, crate::record_fields::number_value(value)))
}
