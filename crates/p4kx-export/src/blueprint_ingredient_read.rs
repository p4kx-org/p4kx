// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;

use crate::blueprint_field_source::value_ref;
use crate::blueprint_presentation::BlueprintIngredient;
use crate::catalog_read::original::OriginalFlowFields;
use crate::crafting_tier::InterpretedTier;
use crate::crafting_types::Cost;
use crate::source_value_catalog::SourceValueRef;
use crate::{CatalogError, ExportAccountingError};

/// Reads the first-tier ingredient order and nearest named selection in one forward pass.
pub(super) fn read(
    first: Option<&InterpretedTier<'_, '_>>,
    fields: &OriginalFlowFields<'_, '_, '_>,
) -> Result<Vec<BlueprintIngredient>, CatalogError> {
    let Some(first) = first else {
        return Ok(Vec::new());
    };
    let mut slots = vec![None; first.costs.len()];
    let mut ingredients = Vec::new();
    for (index, (cost, original)) in first.tier.costs().iter().zip(&first.costs).enumerate() {
        let current = slots
            .get(index)
            .copied()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let fact = fields.fact(original)?;
        match cost {
            Cost::Select(selection) => {
                let slot = if selection.slot.is_some() || selection.slot_key.is_some() {
                    Some(value_ref(fact))
                } else {
                    current
                };
                for child in &selection.options {
                    if child.index() <= index {
                        return Err(ExportAccountingError::ConflictingSourceDatum.into());
                    }
                    let child_slot = slots
                        .get_mut(child.index())
                        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                    *child_slot = slot;
                }
            }
            Cost::Resource(_) | Cost::Item(_) => {
                ingredients.push(BlueprintIngredient {
                    cost: value_ref(fact),
                    slot: current,
                });
            }
        }
    }
    Ok(ingredients)
}

pub(super) fn first_ref(
    first: Option<&InterpretedTier<'_, '_>>,
    fields: &OriginalFlowFields<'_, '_, '_>,
) -> Result<Option<SourceValueRef>, CatalogError> {
    first
        .map(|tier| {
            if !matches!(tier.original, Value::Struct(_)) {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            Ok(value_ref(fields.fact(tier.original)?))
        })
        .transpose()
}
