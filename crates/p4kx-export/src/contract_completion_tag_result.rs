// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourceUseRole, SourceValue, SourcedValue, StructValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use super::value_ref;
use crate::CatalogError;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::schema_fields::every_field_required;
use crate::source_fields::{FieldCapture, FieldError, FieldViolation};
use crate::source_value_catalog::SourceValueRef;

const RESULT: &str = "ContractResult_CompletionTags";
const FIELD: &str = "splitPointsForParty";
const EXPECTED: &str = "a scalar Bool";

/// The authored sharing flag of one original completion-tag result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ContractCompletionTagResult {
    source: SourceValueRef,
    split_points_for_party: SourcedValue<bool>,
}

pub(crate) fn read<'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    capture: &dyn FieldCapture<'source>,
    notes: &mut Vec<FieldError>,
) -> Result<Vec<ContractCompletionTagResult>, CatalogError> {
    let mut rows = Vec::new();
    for result in fields.facts() {
        fields.check_cancel()?;
        let Value::Struct(source) = result.value() else {
            continue;
        };
        if result.field().name() != "contractResults" || source.type_name() != RESULT {
            continue;
        }
        rows.push(ContractCompletionTagResult {
            source: value_ref(result),
            split_points_for_party: split_points(fields, result, source, capture, notes)?,
        });
    }
    Ok(rows)
}

fn split_points<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    owner: &OriginalValueFact<'scope, 'read, 'source>,
    source: &StructValue<'source>,
    capture: &dyn FieldCapture<'source>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<bool>, CatalogError> {
    let kind = owner.observe(OriginalValuePart::ConcreteType, SourceUseRole::Selector)?;
    let Some((_, fact)) = fields.field(Some(owner), FIELD)? else {
        let survey = fields.survey(Some(owner))?;
        return Ok(fields
            .sources()
            .value(SourceValue::Absent, &[kind, survey], true));
    };
    let storage = fact.field().storage();
    let state = if storage.data_type() == DataType::Bool && !storage.is_array() {
        match fact.value() {
            Value::Bool(value) => SourceValue::Present(*value),
            Value::Null => SourceValue::Null,
            value => {
                let path = fields.logical_path(owner)?;
                let violation = FieldViolation::invalid(source, &path, FIELD, value, EXPECTED);
                note(capture, violation, notes)?;
                if matches!(value, Value::Error(_)) {
                    SourceValue::Unreadable
                } else {
                    SourceValue::Invalid
                }
            }
        }
    } else {
        let path = fields.logical_path(owner)?;
        note(
            capture,
            FieldViolation::unsupported(source, &path, fact.field()),
            notes,
        )?;
        SourceValue::Unsupported
    };
    let role = if matches!(state, SourceValue::Present(_)) {
        SourceUseRole::Payload
    } else {
        SourceUseRole::Selector
    };
    let mut datum = fact.bind(OriginalValuePart::Value, state, role, true)?;
    datum.contribute(kind);
    for part in [
        OriginalValuePart::FieldName,
        OriginalValuePart::DataType,
        OriginalValuePart::IsArray,
    ] {
        datum.contribute(fact.observe(part, SourceUseRole::Selector)?);
    }
    Ok(datum.into_cell(fields.sources()))
}

fn note<'source>(
    capture: &dyn FieldCapture<'source>,
    violation: FieldViolation<'_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<(), CatalogError> {
    let body = capture.field_failure(&violation)?;
    notes.push(violation.into_error().with_body(body));
    Ok(())
}

impl ContractCompletionTagResult {
    /// The result's occurrence in its Generator source arena.
    #[must_use]
    pub const fn source(&self) -> SourceValueRef {
        self.source
    }

    /// The original party-sharing flag and its source state.
    #[must_use]
    pub const fn split_points_for_party(&self) -> &SourcedValue<bool> {
        &self.split_points_for_party
    }
}
