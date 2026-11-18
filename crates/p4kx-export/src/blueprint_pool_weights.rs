// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, SourceValue, Value};

use super::{BlueprintPoolRef, BlueprintPoolWeights};
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::resource_float::ResourceFloat32;

pub(super) fn read(
    header: &CanonicalRecordHeader<'_>,
    fields: &OriginalFlowFields<'_, '_, '_>,
    total: f32,
) -> Result<BlueprintPoolWeights, CatalogError> {
    let mut inputs = Vec::new();
    let complete = inputs_for_sum(fields, &mut inputs)?;
    Ok(BlueprintPoolWeights {
        pool: BlueprintPoolRef {
            input_ref: fields.input_ref().clone(),
            record_index: header.record_index.clone(),
            guid: header.guid.clone(),
        },
        total_weight: fields.sources().value(
            SourceValue::Present(ResourceFloat32::new(total)),
            &inputs,
            complete,
        ),
    })
}

fn inputs_for_sum(
    fields: &OriginalFlowFields<'_, '_, '_>,
    inputs: &mut Vec<SourceUse>,
) -> Result<bool, CatalogError> {
    let Some((_, rewards)) = fields.field(None, "blueprintRewards")? else {
        inputs.push(fields.survey(None)?);
        return Ok(true);
    };
    inputs.push(rewards.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?);
    let mut complete = observe(rewards, SourceUseRole::Selector, inputs)?;
    let Value::Array(entries) = rewards.value() else {
        return Ok(complete);
    };
    for value in entries {
        fields.check_cancel()?;
        let entry = fields.fact(value)?;
        complete &= observe(entry, SourceUseRole::Selector, inputs)?;
        if !matches!(value, Value::Struct(_)) {
            continue;
        }
        let Some((_, weight)) = fields.field(Some(entry), "weight")? else {
            inputs.push(fields.survey(Some(entry))?);
            continue;
        };
        inputs.push(weight.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?);
        let role = if crate::record_fields::number_value(weight.value())
            .is_some_and(|value| value > 0.0)
        {
            SourceUseRole::Conversion
        } else {
            SourceUseRole::Selector
        };
        complete &= observe(weight, role, inputs)?;
    }
    Ok(complete)
}

fn observe(
    fact: &crate::catalog_read::original::OriginalValueFact<'_, '_, '_>,
    role: SourceUseRole,
    inputs: &mut Vec<SourceUse>,
) -> Result<bool, CatalogError> {
    let complete = !matches!(fact.value(), Value::Error(_));
    let part = if matches!(fact.value(), Value::Struct(_)) {
        OriginalValuePart::ConcreteType
    } else {
        OriginalValuePart::Value
    };
    inputs.push(fact.observe(
        part,
        if complete {
            role
        } else {
            SourceUseRole::Attempted
        },
    )?);
    Ok(complete)
}
