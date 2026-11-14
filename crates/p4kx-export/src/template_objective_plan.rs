// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;

use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::{CatalogError, ExportAccountingError};

/// An actual objective occurrence with its independent existing semantic selection.
pub(crate) struct TemplateObjectiveSelection<'fields, 'scope, 'read, 'source> {
    pub(crate) fact: &'fields OriginalValueFact<'scope, 'read, 'source>,
    pub(crate) parent: Option<&'fields OriginalValueFact<'scope, 'read, 'source>>,
    pub(crate) selected: bool,
}

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
) -> Result<Vec<TemplateObjectiveSelection<'fields, 'scope, 'read, 'source>>, CatalogError> {
    let mut pending = Vec::new();
    for (index, declaration) in fields
        .declarations(None, "objectiveTokens")?
        .into_iter()
        .enumerate()
    {
        pending.extend(
            fields
                .structs(declaration)?
                .into_iter()
                .map(|fact| (fact, None, index == 0)),
        );
    }
    pending.reverse();
    let mut result = Vec::new();
    while let Some((fact, parent, inherited)) = pending.pop() {
        let Value::Struct(source) = fact.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let selected = inherited
            && crate::record_fields::guid_value(Some(source), "id")
                .and_then(crate::record_fields::nonnil_guid)
                .is_some();
        result.push(TemplateObjectiveSelection {
            fact,
            parent,
            selected,
        });
        if let Some((_, phases)) = fields.field(Some(fact), "childMissionPhases")? {
            pending.extend(
                fields
                    .structs(phases)?
                    .into_iter()
                    .rev()
                    .map(|child| (child, Some(fact), selected)),
            );
        }
    }
    Ok(result)
}
