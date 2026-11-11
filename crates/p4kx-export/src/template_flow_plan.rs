// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::Value;

use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::catalog_template_modules::template_action_layout::{
    ActionLayout, ENTRY_ACTION, RANDOM_ENTRY_TYPE,
};
use crate::{CatalogError, ExportAccountingError};

#[derive(Clone, Copy)]
pub(crate) enum FlowRole {
    Trigger,
    Condition,
    Action,
    RandomPhase,
    RandomActionEntry,
}

pub(crate) struct RoleSelection<'fields, 'scope, 'read, 'source> {
    pub(crate) fact: &'fields OriginalValueFact<'scope, 'read, 'source>,
    pub(crate) role: FlowRole,
    pub(crate) required: bool,
}

pub(crate) struct FlowDeclaration<'fields, 'scope, 'read, 'source> {
    pub(crate) declaration: Option<&'fields OriginalValueFact<'scope, 'read, 'source>>,
    pub(crate) selected: Option<&'fields OriginalValueFact<'scope, 'read, 'source>>,
    pub(crate) triggers: Vec<&'fields OriginalValueFact<'scope, 'read, 'source>>,
}

/// Semantic entry roles from the preserved first-declaration and first-Struct recipes.
pub(crate) struct FlowPlan<'fields, 'scope, 'read, 'source> {
    pub(crate) declarations: Vec<FlowDeclaration<'fields, 'scope, 'read, 'source>>,
    pub(crate) roles: BTreeMap<u64, RoleSelection<'fields, 'scope, 'read, 'source>>,
    pub(crate) semantic_order: Vec<u64>,
}

impl<'fields, 'scope, 'read, 'source> FlowPlan<'fields, 'scope, 'read, 'source> {
    pub(crate) fn read(
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    ) -> Result<Self, CatalogError> {
        let mut result = Self {
            declarations: Vec::new(),
            roles: BTreeMap::new(),
            semantic_order: Vec::new(),
        };
        let declarations = fields.declarations(None, "missionFlow")?;
        let mut pending = Vec::new();
        for (index, declaration) in declarations.into_iter().enumerate() {
            let selected = fields.first_struct(declaration)?;
            let triggers = selected
                .map(|flow| fields.field(Some(flow), "triggers"))
                .transpose()?
                .flatten()
                .map(|(_, fact)| fields.structs(fact))
                .transpose()?
                .unwrap_or_default();
            pending.extend(triggers.iter().map(|fact| RoleSelection {
                fact,
                role: FlowRole::Trigger,
                required: index == 0,
            }));
            result.declarations.push(FlowDeclaration {
                declaration: Some(declaration),
                selected,
                triggers,
            });
        }
        if result.declarations.is_empty() {
            result.declarations.push(FlowDeclaration {
                declaration: None,
                selected: None,
                triggers: Vec::new(),
            });
        }
        pending.reverse();
        while let Some(role) = pending.pop() {
            fields.check_cancel()?;
            children(fields, &role, &mut pending)?;
            result.semantic_order.push(role.fact.ordinal());
            if result.roles.insert(role.fact.ordinal(), role).is_some() {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
        }
        Ok(result)
    }
}

fn children<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    selected: &RoleSelection<'fields, 'scope, 'read, 'source>,
    pending: &mut Vec<RoleSelection<'fields, 'scope, 'read, 'source>>,
) -> Result<(), CatalogError> {
    let Value::Struct(source) = selected.fact.value() else {
        return Ok(());
    };
    match selected.role {
        FlowRole::Action => action_children(fields, selected, source.type_name(), pending),
        FlowRole::RandomActionEntry => entry_child(fields, selected, source.type_name(), pending),
        FlowRole::RandomPhase => Ok(()),
        FlowRole::Trigger | FlowRole::Condition => {
            trigger_or_condition(fields, selected, source.type_name(), pending)
        }
    }
}

fn trigger_or_condition<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    selected: &RoleSelection<'fields, 'scope, 'read, 'source>,
    type_name: &str,
    pending: &mut Vec<RoleSelection<'fields, 'scope, 'read, 'source>>,
) -> Result<(), CatalogError> {
    let mut push = |name: &str, role: FlowRole, many: bool| -> Result<(), CatalogError> {
        let Some((_, field)) = fields.field(Some(selected.fact), name)? else {
            return Ok(());
        };
        let children = if many {
            fields.structs(field)?
        } else {
            fields.first_struct(field)?.into_iter().collect()
        };
        pending.extend(children.into_iter().rev().map(|fact| RoleSelection {
            fact,
            role,
            required: selected.required,
        }));
        Ok(())
    };
    match selected.role {
        FlowRole::Trigger => {
            push("action", FlowRole::Action, false)?;
            push("condition", FlowRole::Condition, false)?;
        }
        FlowRole::Condition => match type_name
            .strip_prefix("MissionFlowCondition_")
            .unwrap_or(type_name)
        {
            "AND" | "OR" => push("conditions", FlowRole::Condition, true)?,
            "NOT" => push("condition", FlowRole::Condition, false)?,
            _ => {}
        },
        _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
    }
    Ok(())
}

fn action_children<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    selected: &RoleSelection<'fields, 'scope, 'read, 'source>,
    type_name: &str,
    pending: &mut Vec<RoleSelection<'fields, 'scope, 'read, 'source>>,
) -> Result<(), CatalogError> {
    let layout = ActionLayout::of(type_name);
    let (array, role) = match layout {
        ActionLayout::PickRandomMissionPhase => (
            fields
                .field(Some(selected.fact), "missionPhases")?
                .map(|(_, field)| field),
            FlowRole::RandomPhase,
        ),
        layout @ (ActionLayout::MultipleActions | ActionLayout::PickRandomAction) => {
            let declared = layout
                .wrapper_field()
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            let array = declared
                .original(fields, selected.fact)?
                .filter(|fact| declared.allows(fact));
            let role = if matches!(layout, ActionLayout::MultipleActions) {
                FlowRole::Action
            } else {
                FlowRole::RandomActionEntry
            };
            (array, role)
        }
        _ => return Ok(()),
    };
    if let Some(array) = array {
        pending.extend(
            fields
                .structs(array)?
                .into_iter()
                .rev()
                .map(|fact| RoleSelection {
                    fact,
                    role,
                    required: selected.required,
                }),
        );
    }
    Ok(())
}

fn entry_child<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    selected: &RoleSelection<'fields, 'scope, 'read, 'source>,
    type_name: &str,
    pending: &mut Vec<RoleSelection<'fields, 'scope, 'read, 'source>>,
) -> Result<(), CatalogError> {
    if type_name == RANDOM_ENTRY_TYPE
        && let Some(fact) = ENTRY_ACTION.original(fields, selected.fact)?
        && ENTRY_ACTION.allows(fact)
        && matches!(fact.value(), Value::Struct(_))
    {
        pending.push(RoleSelection {
            fact,
            role: FlowRole::Action,
            required: selected.required,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "template_flow_plan_tests.rs"]
mod tests;
