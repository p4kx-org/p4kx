// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, Value};

use crate::CatalogError;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::template_reference_index::{TargetSelection, TargetSelectionKind, TemplateReferences};

/// Actual weak slots of the retained source sections, without recursively expanding targets.
pub(crate) struct TemplateWeakSelection<'fields, 'scope, 'read, 'source> {
    pub(crate) caller: &'fields OriginalValueFact<'scope, 'read, 'source>,
    pub(crate) variable: TargetSelection<'fields, 'scope, 'read, 'source>,
    pub(crate) strict_property: bool,
    pub(crate) objective: TargetSelection<'fields, 'scope, 'read, 'source>,
}

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    roots: &[&'fields OriginalValueFact<'scope, 'read, 'source>],
    references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
    properties: &[crate::template_property_plan::SelectedProperty<
        'fields,
        'scope,
        'read,
        'source,
    >],
) -> Result<Vec<TemplateWeakSelection<'fields, 'scope, 'read, 'source>>, CatalogError> {
    let strict = crate::template_strict_property_plan::read(fields, properties)?;
    let mut selected = Vec::new();
    for root in roots {
        for caller in fields
            .facts()
            .get(fields.span(root)?)
            .ok_or(crate::ExportAccountingError::ConflictingSourceDatum)?
        {
            fields.check_cancel()?;
            if (caller.field().storage().data_type() != DataType::WeakPointer
                && !strict.contains(&caller.ordinal()))
                || matches!(caller.value(), Value::Struct(_) | Value::Array(_))
            {
                continue;
            }
            let variable = if strict.contains(&caller.ordinal()) {
                references.select_owned_fact(caller, "missionVariableName")?
            } else {
                references.select_fact(caller, "missionVariableName")?
            };
            let objective = references.select_field(&variable, "id")?;
            selected.push(TemplateWeakSelection {
                caller,
                variable,
                objective,
                strict_property: strict.contains(&caller.ordinal()),
            });
        }
    }
    Ok(selected)
}

pub(crate) fn fields<'selection, 'fields, 'scope, 'read, 'source>(
    selections: &'selection [TemplateWeakSelection<'fields, 'scope, 'read, 'source>],
) -> impl Iterator<Item = &'fields OriginalValueFact<'scope, 'read, 'source>> + 'selection {
    selections
        .iter()
        .flat_map(|selection| [&selection.variable, &selection.objective])
        .filter_map(|selection| match selection.kind() {
            TargetSelectionKind::Instance {
                field: Some(field), ..
            } => Some(field),
            _ => None,
        })
}
