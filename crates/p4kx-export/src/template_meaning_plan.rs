// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;

use crate::CatalogError;
use crate::catalog_read::original::OriginalValueFact;
use crate::template_flow_plan::{FlowPlan, FlowRole};
use crate::template_reference_index::{TargetSelectionKind, TemplateReferences};

/// One selected target field, independent of its final owner-local output index.
pub(crate) struct SelectedTemplateField<'fields, 'scope, 'read, 'source> {
    pub(crate) caller: u64,
    pub(crate) owner: u64,
    pub(crate) field: &'fields OriginalValueFact<'scope, 'read, 'source>,
}

pub(crate) fn targets<'fields, 'scope, 'read, 'source>(
    plan: &FlowPlan<'fields, 'scope, 'read, 'source>,
    references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
) -> Result<Vec<SelectedTemplateField<'fields, 'scope, 'read, 'source>>, CatalogError> {
    let mut selected = Vec::new();
    for role in plan.roles.values() {
        let Value::Struct(source) = role.fact.value() else {
            continue;
        };
        let (caller, target) = match (role.role, source.type_name()) {
            (FlowRole::Condition, "MissionFlowCondition_TokenState" | "TokenState")
            | (FlowRole::Action, "MissionFlowAction_TokenStateChange" | "TokenStateChange") => {
                ("objectiveToken", "id")
            }
            (FlowRole::Condition, "MissionFlowCondition_PropertyIsSet" | "PropertyIsSet") => {
                ("property", "missionVariableName")
            }
            (FlowRole::RandomPhase, _) => ("objective", "id"),
            _ => continue,
        };
        if let TargetSelectionKind::Instance {
            owner,
            field: Some(field),
        } = references.select(role.fact, caller, target)?.kind()
        {
            selected.push(SelectedTemplateField {
                caller: role.fact.ordinal(),
                owner: owner.ordinal(),
                field,
            });
        }
    }
    Ok(selected)
}
