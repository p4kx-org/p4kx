// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, SourceUseRole, SourceValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use super::TemplateDatasetCondition;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::location_condition_kind::LocationConditionKind;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;
use crate::template_source_refs::{DeclarationBinding, SourceFieldRef};
use crate::{CatalogError, ExportAccountingError};

/// Navigates the original authored `MissionItem` selection inputs in the owner's arena.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateMissionItemSelection {
    min_items_to_find: DeclarationBinding,
    max_items_to_find: DeclarationBinding,
    debug_force_chosen_item: DeclarationBinding,
    match_conditions: DeclarationBinding,
}

impl TemplateMissionItemSelection {
    pub(crate) fn read<'scope, 'read, 'source>(
        fields: &OriginalFlowFields<'scope, 'read, 'source>,
        value: &OriginalValueFact<'scope, 'read, 'source>,
        value_ref: impl Fn(
            &OriginalValueFact<'scope, 'read, 'source>,
        ) -> Result<SourceValueRef, CatalogError>,
    ) -> Result<Self, CatalogError> {
        let bind = |name| field(fields, value, name, &value_ref);
        Ok(Self {
            min_items_to_find: bind("minItemsToFind")?,
            max_items_to_find: bind("maxItemsToFind")?,
            debug_force_chosen_item: bind("debugForceChosenItem")?,
            match_conditions: bind("matchConditions")?,
        })
    }
}

/// Navigates every original candidate slot of one `SpecificItems` condition.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateSpecificItems {
    items: DeclarationBinding,
    candidates: Vec<SourceValueRef>,
}

impl TemplateSpecificItems {
    fn read<'scope, 'read, 'source>(
        fields: &OriginalFlowFields<'scope, 'read, 'source>,
        condition: &OriginalValueFact<'scope, 'read, 'source>,
        value_ref: impl Fn(
            &OriginalValueFact<'scope, 'read, 'source>,
        ) -> Result<SourceValueRef, CatalogError>,
    ) -> Result<Self, CatalogError> {
        let items = field(fields, condition, "items", &value_ref)?;
        let mut candidates = Vec::new();
        if let Some((_, array)) = fields.field(Some(condition), "items")?
            && let Value::Array(values) = array.value()
        {
            for value in values {
                fields.check_cancel()?;
                candidates.push(value_ref(fields.fact(value)?)?);
            }
        }
        Ok(Self { items, candidates })
    }
}

fn field<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    parent: &OriginalValueFact<'scope, 'read, 'source>,
    name: &str,
    value_ref: impl FnOnce(
        &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<SourceValueRef, CatalogError>,
) -> Result<DeclarationBinding, CatalogError> {
    Ok(match fields.field(Some(parent), name)? {
        Some((ordinal, _)) => DeclarationBinding::Present {
            field: SourceFieldRef::Field {
                parent: value_ref(parent)?,
                field_index: DecimalU64::new(ordinal),
            },
        },
        None => DeclarationBinding::Absent {
            origin: fields.absent(Some(parent))?,
        },
    })
}

pub(crate) fn dataset_conditions<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    parent: &OriginalValueFact<'scope, 'read, 'source>,
    mission_item: bool,
    value_ref: impl Fn(
        &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<SourceValueRef, CatalogError>,
) -> Result<Vec<TemplateDatasetCondition>, CatalogError> {
    let Some((_, declaration)) = fields.field(Some(parent), "matchConditions")? else {
        return Ok(Vec::new());
    };
    let selected = if matches!(declaration.value(), Value::Struct(_)) {
        vec![declaration]
    } else {
        fields.structs(declaration)?
    };
    selected
        .into_iter()
        .map(|fact| {
            let Value::Struct(source) = fact.value() else {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            };
            let kind = LocationConditionKind::read(source.type_name());
            let specific_items = (mission_item
                && matches!(kind, Some(LocationConditionKind::SpecificItems)))
            .then(|| TemplateSpecificItems::read(fields, fact, &value_ref))
            .transpose()?;
            let mut kind = fact.bind(
                OriginalValuePart::ConcreteType,
                SourceValue::Present(kind),
                SourceUseRole::Selector,
                true,
            )?;
            kind.contribute(
                fact.observe(OriginalValuePart::ConcreteType, SourceUseRole::Conversion)?,
            );
            Ok(TemplateDatasetCondition {
                source: value_ref(fact)?,
                kind: kind.into_cell(fields.sources()),
                specific_items,
            })
        })
        .collect()
}
