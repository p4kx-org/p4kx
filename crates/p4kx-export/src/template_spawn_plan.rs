// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::Value;

use crate::CatalogError;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::mission_flow_owner::FlowOwnerKind;
use crate::template_property_kind::PropertyValueKind;
use crate::template_property_plan::SelectedProperty;

/// An actual root spawn declaration eligible for the established variable-shadowing recipe.
pub(crate) struct TemplateSpawnDeclaration<'fields, 'scope, 'read, 'source> {
    pub(crate) property: &'fields OriginalValueFact<'scope, 'read, 'source>,
    pub(crate) variable: &'fields OriginalValueFact<'scope, 'read, 'source>,
    pub(crate) value: &'fields OriginalValueFact<'scope, 'read, 'source>,
}

/// Actual content reads in root-level, objective and remaining property order, without repeats.
pub(crate) struct TemplateSpawnPlan<'fields, 'scope, 'read, 'source> {
    pub(crate) root: Vec<TemplateSpawnDeclaration<'fields, 'scope, 'read, 'source>>,
    pub(crate) values: Vec<&'fields OriginalValueFact<'scope, 'read, 'source>>,
}

impl<'fields, 'scope, 'read, 'source> TemplateSpawnPlan<'fields, 'scope, 'read, 'source> {
    pub(crate) fn read(
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
        properties: &[SelectedProperty<'fields, 'scope, 'read, 'source>],
        owner: FlowOwnerKind,
    ) -> Result<Self, CatalogError> {
        let mut result = Self {
            root: Vec::new(),
            values: Vec::new(),
        };
        if let Some((_, declaration)) = fields.field(None, owner.properties())? {
            let selected = if matches!(declaration.value(), Value::Struct(_)) {
                vec![declaration]
            } else {
                fields.structs(declaration)?
            };
            for property in selected {
                let Some((_, variable)) = fields.field(Some(property), "missionVariableName")?
                else {
                    continue;
                };
                if !matches!(variable.value(), Value::String(name) | Value::Enum(name) if !name.is_empty())
                {
                    continue;
                }
                let Some(value) = fields.selected_struct(property, "value")? else {
                    continue;
                };
                if is_spawn(value) {
                    result.root.push(TemplateSpawnDeclaration {
                        property,
                        variable,
                        value,
                    });
                }
            }
        }
        let mut seen = HashSet::new();
        for value in result.root.iter().map(|root| root.value).chain(
            properties
                .iter()
                .filter(|property| property.objective.is_some())
                .chain(
                    properties
                        .iter()
                        .filter(|property| property.objective.is_none()),
                )
                .filter_map(|property| property.value),
        ) {
            if is_spawn(value) && seen.insert(value.ordinal()) {
                result.values.push(value);
            }
        }
        Ok(result)
    }
}

fn is_spawn(fact: &OriginalValueFact<'_, '_, '_>) -> bool {
    matches!(fact.value(), Value::Struct(source) if
        PropertyValueKind::read(source.type_name()).is_some_and(PropertyValueKind::is_spawn))
}
