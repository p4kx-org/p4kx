// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{DecimalU64, Value};

use crate::catalog_read::original::OriginalValueFact;
use crate::catalog_template_modules::template_action_layout::{
    ActionLayout, ENTRY_ACTION, ENTRY_PROBABILITY, RANDOM_ENTRY_TYPE,
};
use crate::record_read::ReadError;
use crate::source_fields::FieldCapture;
use crate::template_action_meaning::{ActionMeaning, RandomPhaseMeaning};
use crate::template_flow_plan::{FlowPlan, FlowRole, RoleSelection};
use crate::template_issues::{TemplateError, Unknowns};
use crate::template_reference_index::TemplateReferences;
use crate::template_source::TemplateSourceValue;
use crate::template_source_meaning::{ConditionMeaning, TriggerMeaning};
use crate::template_source_members::TemplateMembers;
use crate::template_source_read;
use crate::{CatalogError, ExportAccountingError};

#[derive(Default)]
struct Meanings {
    condition: Option<ConditionMeaning>,
    action: Option<ActionMeaning>,
    random_phase: Option<RandomPhaseMeaning>,
    trigger: Option<TriggerMeaning>,
}

/// Interpreted Flow roles kept until the shared original inventory is emitted.
pub(crate) struct FlowMeanings<'members, 'fields, 'scope, 'read, 'source> {
    members: &'members TemplateMembers<'fields, 'scope, 'read, 'source>,
    values: BTreeMap<u64, Meanings>,
}

pub(crate) struct PreparedFlow<'members, 'fields, 'scope, 'read, 'source> {
    pub(crate) meanings: FlowMeanings<'members, 'fields, 'scope, 'read, 'source>,
    pub(crate) failure: Option<ReadError>,
}

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    plan: &FlowPlan<'fields, 'scope, 'read, 'source>,
    references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
    unknowns: Option<&Unknowns>,
    capture: &dyn FieldCapture<'source>,
) -> Result<
    (
        crate::template_source::TemplateSource<'source>,
        Option<ReadError>,
    ),
    CatalogError,
> {
    let PreparedFlow { meanings, failure } = prepare(members, plan, references, unknowns, capture)?;
    Ok((meanings.emit(references)?, failure))
}

pub(crate) fn prepare<'members, 'fields, 'scope, 'read, 'source>(
    members: &'members TemplateMembers<'fields, 'scope, 'read, 'source>,
    plan: &FlowPlan<'fields, 'scope, 'read, 'source>,
    references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
    unknowns: Option<&Unknowns>,
    capture: &dyn FieldCapture<'source>,
) -> Result<PreparedFlow<'members, 'fields, 'scope, 'read, 'source>, CatalogError> {
    let mut meanings = BTreeMap::new();
    let mut failure = None;
    for ordinal in &plan.semantic_order {
        let role = plan
            .roles
            .get(ordinal)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        members.fields().check_cancel()?;
        let value = role_meaning(members, references, role, unknowns, capture, &mut failure)?;
        meanings.insert(role.fact.ordinal(), value);
    }
    Ok(PreparedFlow {
        meanings: FlowMeanings {
            members,
            values: meanings,
        },
        failure,
    })
}

fn role_meaning<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
    role: &RoleSelection<'fields, 'scope, 'read, 'source>,
    unknowns: Option<&Unknowns>,
    capture: &dyn FieldCapture<'source>,
    failure: &mut Option<ReadError>,
) -> Result<Meanings, CatalogError> {
    let mut value = Meanings::default();
    match role.role {
        FlowRole::Trigger => {
            value.trigger = Some(crate::template_trigger_read::read(members, role.fact)?);
        }
        FlowRole::Condition => {
            let condition = crate::template_condition_read::read(
                members,
                references,
                role.fact,
                unknowns.filter(|_| role.required && failure.is_none()),
            )?;
            if matches!(condition, ConditionMeaning::Unsupported)
                && role.required
                && failure.is_none()
            {
                *failure = Some(unknown(
                    "missionFlow condition",
                    role.fact.value(),
                    capture,
                )?);
            }
            value.condition = Some(condition);
        }
        FlowRole::Action => {
            let action = crate::template_action_read::action(
                members,
                references,
                role.fact,
                unknowns.filter(|_| role.required && failure.is_none()),
            )?;
            if role.required && failure.is_none() {
                *failure = action_failure(members, role.fact, &action, capture)?;
            }
            value.action = Some(action);
        }
        FlowRole::RandomPhase => {
            value.random_phase = Some(crate::template_action_read::phase(
                members, references, role.fact,
            )?);
        }
        FlowRole::RandomActionEntry => {
            if role.required && failure.is_none() {
                *failure = entry_failure(members, role.fact, capture)?;
            }
        }
    }
    Ok(value)
}

fn action_failure<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    fact: &OriginalValueFact<'scope, 'read, 'source>,
    action: &ActionMeaning,
    capture: &dyn FieldCapture<'source>,
) -> Result<Option<ReadError>, CatalogError> {
    if matches!(action, ActionMeaning::Unsupported) {
        return unknown("missionFlow action", fact.value(), capture).map(Some);
    }
    let Value::Struct(source) = fact.value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    ActionLayout::of(source.type_name())
        .wrapper_field()
        .map_or_else(
            || Ok(None),
            |declaration| declaration.failure(members.fields(), fact, capture),
        )
}

fn entry_failure<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    fact: &OriginalValueFact<'scope, 'read, 'source>,
    capture: &dyn FieldCapture<'source>,
) -> Result<Option<ReadError>, CatalogError> {
    let Value::Struct(source) = fact.value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    if source.type_name() != RANDOM_ENTRY_TYPE {
        return unknown("missionFlow random action entry", fact.value(), capture).map(Some);
    }
    for declaration in [ENTRY_ACTION, ENTRY_PROBABILITY] {
        if let Some(error) = declaration.failure(members.fields(), fact, capture)? {
            return Ok(Some(error));
        }
    }
    Ok(None)
}

impl<'members, 'fields, 'scope, 'read, 'source>
    FlowMeanings<'members, 'fields, 'scope, 'read, 'source>
{
    pub(crate) const fn members(
        &self,
    ) -> &'members TemplateMembers<'fields, 'scope, 'read, 'source> {
        self.members
    }

    pub(crate) fn emit(
        self,
        references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
    ) -> Result<crate::template_source::TemplateSource<'source>, CatalogError> {
        let Self {
            members,
            values: mut meanings,
        } = self;
        let source = template_source_read::read(members, references, |fact, original| {
            let meaning = meanings.remove(&fact.ordinal()).unwrap_or_default();
            Ok(TemplateSourceValue {
                source_ordinal: DecimalU64::new(fact.ordinal()),
                original,
                condition: meaning.condition,
                action: meaning.action,
                random_phase: meaning.random_phase,
                trigger: meaning.trigger,
            })
        })?;
        if !meanings.is_empty() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(source)
    }
}

fn unknown<'source>(
    field: &'static str,
    value: &Value<'source>,
    capture: &dyn FieldCapture<'source>,
) -> Result<ReadError, CatalogError> {
    match value {
        Value::Struct(source) => {
            let body = capture.unknown_type(source, field)?;
            Ok(Box::new(
                TemplateError::unknown(field, source).with_body(body),
            ))
        }
        _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
    }
}
