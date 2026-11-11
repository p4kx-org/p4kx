// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue, SourcedValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use crate::ExportAccountingError;
use crate::broker_values::Comparison;
use crate::catalog_read::original::{OriginalValueFact, OriginalValuePart};
use crate::record_read::ReadError;
use crate::schema_fields::every_field_required;
use crate::source_fields::{FieldCapture, unknown_struct};
use crate::source_value_catalog::SourceValueRef;
use crate::template_issues::Unknowns;
use crate::template_source_members::TemplateMembers;

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum StandingOperator {
    Requirement,
    And,
    LeftParenthesis,
    RightParenthesis,
}

/// A selected infix term linked to its actual original payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BrokerStandingTerm {
    pub(crate) source: SourceValueRef,
    pub(crate) operator: SourcedValue<StandingOperator>,
    pub(crate) comparison: Option<SourcedValue<Option<Comparison>>>,
}

pub(crate) fn read<'source>(
    members: &TemplateMembers<'_, '_, '_, 'source>,
    unknowns: &Unknowns,
    capture: &dyn FieldCapture<'source>,
) -> Result<Vec<BrokerStandingTerm>, ReadError> {
    let fields = members.fields();
    let Some((_, declaration)) = fields.field(None, "reputationRequirements")? else {
        return Ok(Vec::new());
    };
    let Some(block) = fields.first_struct(declaration)? else {
        return Ok(Vec::new());
    };
    let Value::Struct(source) = block.value() else {
        return Err(Box::new(crate::CatalogError::from(
            ExportAccountingError::ConflictingSourceDatum,
        )));
    };
    if source.type_name() != "SReputationMissionRequirementsParams" {
        return Err(unknown_struct(
            source,
            "reputationRequirements",
            Some(capture),
        ));
    }
    let Some((_, expression)) = fields.field(Some(block), "expression")? else {
        return Ok(Vec::new());
    };
    fields
        .structs(expression)?
        .into_iter()
        .map(|fact| term(members, unknowns, capture, fact))
        .collect()
}

fn term<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    unknowns: &Unknowns,
    capture: &dyn FieldCapture<'source>,
    fact: &'fields OriginalValueFact<'scope, 'read, 'source>,
) -> Result<BrokerStandingTerm, ReadError> {
    let Value::Struct(source) = fact.value() else {
        return Err(Box::new(crate::CatalogError::from(
            ExportAccountingError::ConflictingSourceDatum,
        )));
    };
    let operator = match source.type_name() {
        "SReputationMissionGiverRequirementParams" => StandingOperator::Requirement,
        "SReputationMissionRequirementExpression_And" => StandingOperator::And,
        "SReputationMissionRequirementExpression_LeftParenthesis" => {
            StandingOperator::LeftParenthesis
        }
        "SReputationMissionRequirementExpression_RightParenthesis" => {
            StandingOperator::RightParenthesis
        }
        _ => {
            return Err(unknown_struct(
                source,
                "reputationRequirements expression",
                Some(capture),
            ));
        }
    };
    let comparison = match operator {
        StandingOperator::Requirement => comparison(members, fact, unknowns)?,
        _ => None,
    };
    let mut value = fact.bind(
        OriginalValuePart::ConcreteType,
        SourceValue::Present(operator),
        SourceUseRole::Selector,
        true,
    )?;
    value.contribute(fact.observe(OriginalValuePart::ConcreteType, SourceUseRole::Conversion)?);
    Ok(BrokerStandingTerm {
        source: members.value_ref(fact)?,
        operator: value.into_cell(members.fields().sources()),
        comparison,
    })
}

fn comparison<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    term: &OriginalValueFact<'scope, 'read, 'source>,
    unknowns: &Unknowns,
) -> Result<Option<SourcedValue<Option<Comparison>>>, crate::CatalogError> {
    let fields = members.fields();
    let Some((_, fact)) = fields.field(Some(term), "comparison")? else {
        return Ok(None);
    };
    let (value, role, complete) = match fact.value() {
        Value::Error(_) => (SourceValue::Unreadable, SourceUseRole::Attempted, false),
        Value::String(text) | Value::Enum(text) if !text.is_empty() => (
            SourceValue::Present(unknowns.parse("comparison", Some(text), Comparison::parse)),
            SourceUseRole::Payload,
            true,
        ),
        _ => (SourceValue::Present(None), SourceUseRole::Selector, true),
    };
    let mut value = fact.bind(OriginalValuePart::Value, value, role, complete)?;
    if complete {
        value.contribute(fact.observe(OriginalValuePart::Value, SourceUseRole::Conversion)?);
    }
    Ok(Some(value.into_cell(fields.sources())))
}
