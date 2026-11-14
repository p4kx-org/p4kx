// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::CatalogError;
use crate::catalog_read::original::OriginalValueFact;
use crate::template_property_plan::SelectedProperty;
use crate::template_reference_index::{TargetSelection, TargetSelectionKind, TemplateReferences};

/// Genuine same-frame output selections derived once from the actual weak target.
pub(crate) struct TemplateOutputSelection<'fields, 'scope, 'read, 'source> {
    pub(crate) caller: &'fields OriginalValueFact<'scope, 'read, 'source>,
    pub(crate) variable: TargetSelection<'fields, 'scope, 'read, 'source>,
    pub(crate) objective: TargetSelection<'fields, 'scope, 'read, 'source>,
}

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    fields: &'fields crate::catalog_read::original::OriginalFlowFields<'scope, 'read, 'source>,
    references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
    properties: &[SelectedProperty<'fields, 'scope, 'read, 'source>],
) -> Result<Vec<TemplateOutputSelection<'fields, 'scope, 'read, 'source>>, CatalogError> {
    let mut selected = Vec::new();
    for property in properties
        .iter()
        .filter(|property| property.objective.is_some())
    {
        let Some((_, caller)) = fields.field(Some(property.property), "linkedOutput")? else {
            continue;
        };
        let target = references.select_fact(caller, "property")?;
        let variable = references.nested_field(&target, "property", "missionVariableName")?;
        let objective = references.ancestor_field(&target, "properties", "id")?;
        selected.push(TemplateOutputSelection {
            caller,
            variable,
            objective,
        });
    }
    Ok(selected)
}

pub(crate) fn fields<'selection, 'fields, 'scope, 'read, 'source>(
    selected: &'selection [TemplateOutputSelection<'fields, 'scope, 'read, 'source>],
) -> impl Iterator<Item = &'fields OriginalValueFact<'scope, 'read, 'source>> + 'selection {
    selected
        .iter()
        .flat_map(|selection| [&selection.variable, &selection.objective])
        .flat_map(|selection| {
            [
                selection.retained_root(),
                match selection.kind() {
                    TargetSelectionKind::Instance { field, .. } => field,
                    _ => None,
                },
            ]
        })
        .flatten()
}
