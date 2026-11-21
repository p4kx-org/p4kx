// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;

use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::catalog_template_modules::template_action_layout::{
    ActionField, ActionLayout, ENTRY_ACTION, ENTRY_PROBABILITY, RANDOM_ENTRY_TYPE,
};
use crate::source_value_catalog::SourceValueRef;
use crate::template_action_meaning::{ActionMeaning, ProbabilityInterpretation, RandomActionEntry};
use crate::template_field_binding;
use crate::template_source_members::TemplateMembers;
use crate::{CatalogError, ExportAccountingError};

pub(crate) fn read<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    fact: &OriginalValueFact<'scope, 'read, 'source>,
    layout: ActionLayout,
) -> Result<ActionMeaning, CatalogError> {
    let declaration = layout
        .wrapper_field()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let fields = members.fields();
    let actions_field = template_field_binding::read(fields, fact, declaration.name())?;
    let actions = match declaration.original(fields, fact)? {
        Some(array) => fields.elements(array)?,
        None => Vec::new(),
    };
    match layout {
        ActionLayout::MultipleActions => Ok(ActionMeaning::MultipleActions {
            actions_field,
            actions: actions
                .into_iter()
                .map(|action| members.value_ref(action))
                .collect::<Result<_, _>>()?,
        }),
        ActionLayout::PickRandomAction => Ok(ActionMeaning::PickRandomAction {
            actions_field,
            entries: actions
                .into_iter()
                .map(|entry| read_entry(members, entry))
                .collect::<Result<_, _>>()?,
        }),
        _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
    }
}

fn read_entry<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    fact: &OriginalValueFact<'scope, 'read, 'source>,
) -> Result<RandomActionEntry, CatalogError> {
    let value = members.value_ref(fact)?;
    let fields = members.fields();
    Ok(match fact.value() {
        Value::Struct(source) if source.type_name() == RANDOM_ENTRY_TYPE => {
            RandomActionEntry::Declared {
                value,
                action_field: template_field_binding::read(fields, fact, ENTRY_ACTION.name())?,
                action: reference(members, fact, ENTRY_ACTION)?,
                probability_field: template_field_binding::read(
                    fields,
                    fact,
                    ENTRY_PROBABILITY.name(),
                )?,
                probability_interpretation: probability(fields, fact)?,
            }
        }
        Value::Struct(_) => RandomActionEntry::Unsupported { value },
        Value::Null => RandomActionEntry::Null { value },
        Value::Error(_) => RandomActionEntry::Unreadable { value },
        _ => RandomActionEntry::Invalid { value },
    })
}

fn reference<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    owner: &OriginalValueFact<'scope, 'read, 'source>,
    declaration: ActionField,
) -> Result<Option<SourceValueRef>, CatalogError> {
    declaration
        .original(members.fields(), owner)?
        .map(|child| members.value_ref(child))
        .transpose()
}

fn probability<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    owner: &OriginalValueFact<'scope, 'read, 'source>,
) -> Result<ProbabilityInterpretation, CatalogError> {
    let original = ENTRY_PROBABILITY.original(fields, owner)?;
    Ok(match original.map(OriginalValueFact::value) {
        None => ProbabilityInterpretation::Absent,
        Some(Value::Null) => ProbabilityInterpretation::Null,
        Some(Value::Error(_)) => ProbabilityInterpretation::Unreadable,
        Some(Value::Single(value)) if value.is_finite() => ProbabilityInterpretation::Finite,
        Some(Value::Single(_)) => ProbabilityInterpretation::Nonfinite,
        _ => ProbabilityInterpretation::WrongShape,
    })
}
