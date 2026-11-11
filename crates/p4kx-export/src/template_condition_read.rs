// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;

use crate::catalog_read::original::OriginalValueFact;
use crate::template_binding_read;
use crate::template_issues::Unknowns;
use crate::template_meaning_fields;
use crate::template_reference_index::TemplateReferences;
use crate::template_source_meaning::ConditionMeaning;
use crate::template_source_members::TemplateMembers;
use crate::{CatalogError, ExportAccountingError};

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
    fact: &'fields OriginalValueFact<'scope, 'read, 'source>,
    unknowns: Option<&Unknowns>,
) -> Result<ConditionMeaning, CatalogError> {
    let Value::Struct(source) = fact.value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let fields = members.fields();
    let state = || template_meaning_fields::state(fields, fact, "tokenState", unknowns);
    let state_field = || crate::template_field_binding::read(fields, fact, "tokenState");
    Ok(
        match source
            .type_name()
            .strip_prefix("MissionFlowCondition_")
            .unwrap_or_else(|| source.type_name())
        {
            "TokenState" => ConditionMeaning::TokenState {
                objective_token_field: crate::template_field_binding::read(
                    fields,
                    fact,
                    "objectiveToken",
                )?,
                objective: template_binding_read::objective(
                    members,
                    references,
                    fact,
                    "objectiveToken",
                )?,
                token_state_field: state_field()?,
                state: state()?,
            },
            "AllTokensState" => ConditionMeaning::AllTokensState {
                token_state_field: state_field()?,
                state: state()?,
            },
            "AnyTokensState" => ConditionMeaning::AnyTokensState {
                token_state_field: state_field()?,
                state: state()?,
            },
            "OnMissionStart" => ConditionMeaning::OnMissionStart,
            "AND" => ConditionMeaning::And {
                conditions_field: crate::template_field_binding::read(fields, fact, "conditions")?,
                children: children(members, fact)?,
            },
            "OR" => ConditionMeaning::Or {
                conditions_field: crate::template_field_binding::read(fields, fact, "conditions")?,
                children: children(members, fact)?,
            },
            "NOT" => ConditionMeaning::Not {
                condition_field: crate::template_field_binding::read(fields, fact, "condition")?,
                child: fields
                    .selected_struct(fact, "condition")?
                    .map(|child| members.value_ref(child))
                    .transpose()?,
            },
            "PropertyIsSet" => ConditionMeaning::PropertyIsSet {
                property_field: crate::template_field_binding::read(fields, fact, "property")?,
                property: template_binding_read::property(members, references, fact)?,
            },
            _ => ConditionMeaning::Unsupported,
        },
    )
}

fn children<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    fact: &OriginalValueFact<'scope, 'read, 'source>,
) -> Result<Vec<crate::source_value_catalog::SourceValueRef>, CatalogError> {
    let fields = members.fields();
    match fields.field(Some(fact), "conditions")? {
        Some((_, field)) => fields
            .elements(field)?
            .into_iter()
            .map(|child| members.value_ref(child))
            .collect(),
        None => Ok(Vec::new()),
    }
}
