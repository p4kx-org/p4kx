// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;

use crate::CatalogError;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::mission_flow_owner::FlowOwnerKind;
use crate::template_objective_plan::TemplateObjectiveSelection;

pub(crate) struct SelectedProperty<'fields, 'scope, 'read, 'source> {
    pub(crate) property: &'fields OriginalValueFact<'scope, 'read, 'source>,
    pub(crate) objective: Option<&'fields OriginalValueFact<'scope, 'read, 'source>>,
    pub(crate) naming: &'fields OriginalValueFact<'scope, 'read, 'source>,
    pub(crate) value: Option<&'fields OriginalValueFact<'scope, 'read, 'source>>,
}

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    objectives: &[TemplateObjectiveSelection<'fields, 'scope, 'read, 'source>],
    owner: FlowOwnerKind,
) -> Result<Vec<SelectedProperty<'fields, 'scope, 'read, 'source>>, CatalogError> {
    let mut selected = Vec::new();
    if let Some((_, declaration)) = fields.field(None, owner.properties())? {
        for property in fields.structs(declaration)? {
            let value = fields.selected_struct(property, "value")?;
            selected.push(SelectedProperty {
                property,
                objective: None,
                naming: property,
                value,
            });
        }
    }
    for objective in objectives.iter().filter(|objective| objective.selected) {
        let Some((_, declaration)) = fields.field(Some(objective.fact), "properties")? else {
            continue;
        };
        let properties = if matches!(declaration.value(), Value::Struct(_)) {
            vec![declaration]
        } else {
            fields.structs(declaration)?
        };
        for property in properties {
            let embedded = fields.selected_struct(property, "property")?;
            let value = embedded
                .map(|embedded| fields.selected_struct(embedded, "value"))
                .transpose()?
                .flatten();
            selected.push(SelectedProperty {
                property,
                objective: Some(objective.fact),
                naming: embedded.unwrap_or(property),
                value,
            });
        }
    }
    Ok(selected)
}
