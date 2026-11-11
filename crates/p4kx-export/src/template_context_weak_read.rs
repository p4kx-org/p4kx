// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::CatalogError;
use crate::template_projection::{TemplateObjectiveTarget, TemplatePropertyBinding};
use crate::template_reference_index::TemplateReferences;
use crate::template_source_members::TemplateMembers;
use crate::template_weak_plan::TemplateWeakSelection;

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &TemplateReferences<'fields, 'scope, 'read, 'source>,
    selected: &[TemplateWeakSelection<'fields, 'scope, 'read, 'source>],
) -> Result<(Vec<TemplatePropertyBinding>, Vec<TemplateObjectiveTarget>), CatalogError> {
    let mut properties = Vec::with_capacity(selected.len());
    let mut objectives = Vec::with_capacity(selected.len());
    for selection in selected {
        let caller = members.value_ref(selection.caller)?;
        properties.push(TemplatePropertyBinding {
            caller,
            target: crate::template_binding_read::property_selected(
                members,
                references,
                &selection.variable,
            )?,
            mission_property_status: if selection.strict_property {
                crate::template_strict_property_read::read(
                    members,
                    references,
                    &selection.variable,
                )?
            } else {
                None
            },
        });
        objectives.push(TemplateObjectiveTarget {
            caller,
            target: crate::template_binding_read::objective_selected(
                members,
                references,
                &selection.objective,
            )?,
        });
    }
    Ok((properties, objectives))
}
