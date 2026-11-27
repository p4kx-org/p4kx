// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;

use crate::CatalogError;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::template_action_meaning::ProbabilityInterpretation;
use crate::template_issues::Unknowns;
use crate::template_objective::ObjectiveState;
use crate::template_source_meaning::{NoteInterpretation, StateInterpretation};
use crate::template_source_members::TemplateMembers;

pub(crate) fn state<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    parent: &OriginalValueFact<'scope, 'read, 'source>,
    name: &str,
    unknowns: Option<&Unknowns>,
) -> Result<StateInterpretation, CatalogError> {
    let Some((_, fact)) = members.fields().field(Some(parent), name)? else {
        return Ok(StateInterpretation::Absent);
    };
    Ok(match fact.value() {
        Value::Null => StateInterpretation::Null,
        Value::Error(_) => StateInterpretation::Unreadable,
        Value::String(value) | Value::Enum(value) => {
            let parsed = match unknowns {
                Some(unknowns) => {
                    members.parse_warning(unknowns, "flow state", fact, ObjectiveState::parse)?
                }
                None if value.is_empty() => None,
                None => ObjectiveState::parse(value),
            };
            parsed.map_or(StateInterpretation::Unknown, |state| {
                StateInterpretation::Known { state }
            })
        }
        _ => StateInterpretation::WrongShape,
    })
}

pub(crate) fn note<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    parent: &OriginalValueFact<'scope, 'read, 'source>,
) -> Result<NoteInterpretation, CatalogError> {
    Ok(
        match fields
            .field(Some(parent), "description")?
            .map(|(_, fact)| fact.value())
        {
            None => NoteInterpretation::Absent,
            Some(Value::Null) => NoteInterpretation::Null,
            Some(Value::Error(_)) => NoteInterpretation::Unreadable,
            Some(Value::String("") | Value::Enum("")) => NoteInterpretation::Empty,
            Some(Value::String(_) | Value::Enum(_)) => NoteInterpretation::Value,
            _ => NoteInterpretation::WrongShape,
        },
    )
}

pub(crate) fn probability<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    parent: &OriginalValueFact<'scope, 'read, 'source>,
) -> Result<ProbabilityInterpretation, CatalogError> {
    let Some((_, fact)) = fields.field(Some(parent), "probability")? else {
        return Ok(ProbabilityInterpretation::Absent);
    };
    let Value::Struct(source) = parent.value() else {
        return Err(crate::ExportAccountingError::ConflictingSourceDatum.into());
    };
    if crate::record_fields::number(Some(source), "probability").is_some() {
        return Ok(ProbabilityInterpretation::Finite);
    }
    Ok(match fact.value() {
        Value::Null => ProbabilityInterpretation::Null,
        Value::Error(_) => ProbabilityInterpretation::Unreadable,
        Value::Single(value) if !value.is_finite() => ProbabilityInterpretation::Nonfinite,
        Value::Double(value) if !value.is_finite() => ProbabilityInterpretation::Nonfinite,
        Value::Single(_) | Value::Double(_) | Value::UInt64(_) => {
            ProbabilityInterpretation::OutOfRange
        }
        _ => ProbabilityInterpretation::WrongShape,
    })
}
