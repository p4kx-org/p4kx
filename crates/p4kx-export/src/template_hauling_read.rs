// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::Value;

use crate::catalog_read::original::OriginalValueFact;
use crate::contract_hauling::{HaulingItemKind, HaulingSource};
use crate::mission_flow_owner::FlowOwnerKind;
use crate::template_hauling_projection::{
    TemplateHaulingItem, TemplateHaulingOrder, TemplateHaulingRequirement,
};
use crate::template_source_members::TemplateMembers;
use crate::{CatalogError, ExportAccountingError};

pub(crate) fn read(
    members: &TemplateMembers<'_, '_, '_, '_>,
    owner: FlowOwnerKind,
) -> Result<Vec<TemplateHaulingOrder>, CatalogError> {
    let fields = members.fields();
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    if matches!(owner, FlowOwnerKind::ContractTemplate)
        && let Some((_, properties)) = fields.field(None, owner.properties())?
    {
        for property in fields.structs(properties)? {
            let Some((_, value)) = fields.field(Some(property), "value")? else {
                continue;
            };
            if !is_type(value, "MissionPropertyValue_HaulingOrders") {
                continue;
            }
            let variable = fields.field(Some(property), "missionVariableName")?;
            let name = variable.and_then(|(_, fact)| text(fact.value()));
            if name.is_some_and(|name| !seen.insert(name)) {
                continue;
            }
            result.push(TemplateHaulingOrder {
                source: HaulingSource::Template,
                property: Some(members.value_ref(property)?),
                variable: variable
                    .map(|(_, fact)| members.field_ref(fact))
                    .transpose()?,
                requirements: requirements(members, value, "haulingOrderContent", false)?,
            });
        }
    }
    let mut objective = Vec::new();
    if let Some((_, tokens)) = fields.field(None, "objectiveTokens")? {
        for token in fields.structs(tokens)? {
            let Some((_, handler)) = fields.field(Some(token), "objectiveHandler")? else {
                continue;
            };
            if is_type(handler, "ObjectiveHandler_Hauling") {
                objective.extend(requirements(members, handler, "haulingOrders", true)?);
            }
        }
    }
    if !objective.is_empty() {
        result.push(TemplateHaulingOrder {
            source: owner.objective_hauling(),
            property: None,
            variable: None,
            requirements: objective,
        });
    }
    Ok(result)
}

fn requirements<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    parent: &OriginalValueFact<'scope, 'read, 'source>,
    name: &str,
    objective: bool,
) -> Result<Vec<TemplateHaulingRequirement>, CatalogError> {
    let fields = members.fields();
    let Some((_, list)) = fields.field(Some(parent), name)? else {
        return Ok(Vec::new());
    };
    let mut requirements = Vec::new();
    for content in fields.structs(list)? {
        if is_type(content, "HaulingOrderContent_Or") {
            let mut options = Vec::new();
            if let Some((_, array)) = fields.field(Some(content), "options")? {
                for option in fields.structs(array)? {
                    options.push(items(members, option, objective)?);
                }
            }
            requirements.push(TemplateHaulingRequirement { options });
        } else if let Some(item) = item(members, content, objective)? {
            requirements.push(TemplateHaulingRequirement {
                options: vec![vec![item]],
            });
        }
    }
    Ok(requirements)
}

fn items<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    option: &OriginalValueFact<'scope, 'read, 'source>,
    objective: bool,
) -> Result<Vec<TemplateHaulingItem>, CatalogError> {
    let fields = members.fields();
    let Some((_, orders)) = fields.field(Some(option), "orders")? else {
        return Ok(Vec::new());
    };
    let mut result = Vec::new();
    for order in fields.structs(orders)? {
        if let Some(item) = item(members, order, objective)? {
            result.push(item);
        }
    }
    Ok(result)
}

fn item<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    fact: &OriginalValueFact<'scope, 'read, 'source>,
    objective: bool,
) -> Result<Option<TemplateHaulingItem>, CatalogError> {
    let Value::Struct(source) = fact.value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let Some((_, suffix)) = source.type_name().split_once('_') else {
        return Ok(None);
    };
    let kind = match suffix {
        "Resource" | "ResourceUnlimitedDropOff" => HaulingItemKind::Resource,
        "EntityClass" => HaulingItemKind::EntityClass,
        "EntityClasses" => HaulingItemKind::EntityClasses,
        "MissionItem" if !objective => HaulingItemKind::MissionItem,
        _ => return Ok(None),
    };
    Ok(Some(TemplateHaulingItem {
        source: members.value_ref(fact)?,
        kind,
    }))
}

fn is_type(fact: &OriginalValueFact<'_, '_, '_>, name: &str) -> bool {
    matches!(fact.value(), Value::Struct(source) if source.type_name() == name)
}

const fn text<'source>(value: &Value<'source>) -> Option<&'source str> {
    match value {
        Value::String(value) | Value::Enum(value) if !value.is_empty() => Some(value),
        _ => None,
    }
}
