// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;

use crate::catalog_read::original::OriginalValueFact;
use crate::catalog_template_modules::template_action_layout::ActionLayout;
use crate::template_action_meaning::{ActionMeaning, RandomPhaseMeaning};
use crate::template_binding_read;
use crate::template_issues::Unknowns;
use crate::template_meaning_fields;
use crate::template_reference_index::TemplateReferences;
use crate::template_source_members::TemplateMembers;
use crate::{CatalogError, ExportAccountingError};

pub(crate) fn action<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
    fact: &'fields OriginalValueFact<'scope, 'read, 'source>,
    unknowns: Option<&Unknowns>,
) -> Result<ActionMeaning, CatalogError> {
    let Value::Struct(source) = fact.value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let fields = members.fields();
    let state = || template_meaning_fields::state(members, fact, "toState", unknowns);
    let state_field = || crate::template_field_binding::read(fields, fact, "toState");
    Ok(match ActionLayout::of(source.type_name()) {
        ActionLayout::StateChange => ActionMeaning::StateChange {
            to_state_field: state_field()?,
            state: state()?,
        },
        ActionLayout::TokenStateChange => ActionMeaning::TokenStateChange {
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
            to_state_field: state_field()?,
            state: state()?,
        },
        ActionLayout::PickRandomMissionPhase => ActionMeaning::PickRandomMissionPhase {
            to_state_field: state_field()?,
            state: state()?,
            mission_phases_field: crate::template_field_binding::read(
                fields,
                fact,
                "missionPhases",
            )?,
            phases: match fields.field(Some(fact), "missionPhases")? {
                Some((_, field)) => fields
                    .structs(field)?
                    .into_iter()
                    .map(|phase| members.value_ref(phase))
                    .collect::<Result<_, _>>()?,
                None => Vec::new(),
            },
        },
        layout @ (ActionLayout::MultipleActions | ActionLayout::PickRandomAction) => {
            return crate::catalog_template_modules::template_action_wrappers::read(
                members, fact, layout,
            );
        }
        ActionLayout::Unknown => ActionMeaning::Unsupported,
    })
}

pub(crate) fn phase<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
    fact: &'fields OriginalValueFact<'scope, 'read, 'source>,
) -> Result<RandomPhaseMeaning, CatalogError> {
    let fields = members.fields();
    Ok(RandomPhaseMeaning {
        objective_field: crate::template_field_binding::read(fields, fact, "objective")?,
        objective: template_binding_read::objective(members, references, fact, "objective")?,
        probability_field: crate::template_field_binding::read(fields, fact, "probability")?,
        probability_interpretation: template_meaning_fields::probability(fields, fact)?,
    })
}
