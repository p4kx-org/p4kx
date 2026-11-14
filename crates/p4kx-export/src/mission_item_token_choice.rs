// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, Value};
use schemars::JsonSchema;
use serde::Serialize;

use super::evidence::{Decisions, Fact, Fields};
use crate::CatalogError;
use crate::catalog_read::original::OriginalValuePart;

const SPECIFIC_ITEMS: &str = "DataSetMatchCondition_SpecificItemsDef";

/// A reason why the authored `MissionItem` token cannot be rendered statically.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum MissionItemGap {
    RuntimeQuery,
    EmptyCandidates,
    AmbiguousCandidates,
    MissingItem,
    MissingVariant,
    AmbiguousVariants,
    MissingTranslation,
}

#[derive(Debug)]
pub(crate) enum MissionItemChoice {
    Static { guid: Guid, record_index: u32 },
    Unresolved(MissionItemGap),
}

pub(super) fn select<'fields, 'scope, 'read, 'source>(
    fields: &Fields<'fields, 'scope, 'read, 'source>,
    property: Fact<'fields, 'scope, 'read, 'source>,
    evidence: &mut Decisions,
) -> Result<MissionItemChoice, CatalogError> {
    use MissionItemChoice::Unresolved;

    let mut selection = Unresolved(MissionItemGap::RuntimeQuery);
    if let Some(conditions) = fields.field(Some(property), "matchConditions", evidence)? {
        let conditions = fields.items(conditions)?;
        for condition in &conditions {
            fields.record(condition, original_part(condition.value()), evidence)?;
        }
        if let [condition] = conditions.as_slice()
            && matches!(condition.value(), Value::Struct(condition) if condition.type_name() == SPECIFIC_ITEMS)
        {
            selection = specific(fields, condition, evidence)?;
        }
    }
    if matches!(selection, MissionItemChoice::Static { .. })
        && !exactly_one_item(fields, property, evidence)?
    {
        return Ok(Unresolved(MissionItemGap::RuntimeQuery));
    }
    Ok(selection)
}

fn exactly_one_item<'fields, 'scope, 'read, 'source>(
    fields: &Fields<'fields, 'scope, 'read, 'source>,
    property: Fact<'fields, 'scope, 'read, 'source>,
    evidence: &mut Decisions,
) -> Result<bool, CatalogError> {
    let min = fields.field(Some(property), "minItemsToFind", evidence)?;
    let max = fields.field(Some(property), "maxItemsToFind", evidence)?;
    let debug = fields.field(Some(property), "debugForceChosenItem", evidence)?;
    Ok([min, max]
        .into_iter()
        .all(|bound| bound.is_some_and(|bound| matches!(bound.value(), Value::Int32(1))))
        && debug.is_none_or(|fact| matches!(fact.value(), Value::Null)))
}

fn specific<'fields, 'scope, 'read, 'source>(
    fields: &Fields<'fields, 'scope, 'read, 'source>,
    condition: Fact<'fields, 'scope, 'read, 'source>,
    evidence: &mut Decisions,
) -> Result<MissionItemChoice, CatalogError> {
    use MissionItemChoice::Unresolved;

    let Some(items) = fields.field(Some(condition), "items", evidence)? else {
        return Ok(Unresolved(MissionItemGap::RuntimeQuery));
    };
    let Value::Array(values) = items.value() else {
        return Ok(Unresolved(MissionItemGap::RuntimeQuery));
    };
    Ok(match &**values {
        [] => Unresolved(MissionItemGap::EmptyCandidates),
        [item] => {
            let candidates = fields.items(items)?;
            let [candidate] = candidates.as_slice() else {
                return Err(CatalogError::MissionTextPlan);
            };
            fields.record(candidate, OriginalValuePart::Value, evidence)?;
            match item {
                Value::Reference(target) if target.location().is_none() => {
                    MissionItemChoice::Static {
                        guid: target.guid(),
                        record_index: target.record_index(),
                    }
                }
                _ => Unresolved(MissionItemGap::MissingItem),
            }
        }
        _ => Unresolved(MissionItemGap::AmbiguousCandidates),
    })
}

const fn original_part(value: &Value<'_>) -> OriginalValuePart {
    if matches!(value, Value::Struct(_)) {
        OriginalValuePart::ConcreteType
    } else {
        OriginalValuePart::Value
    }
}
