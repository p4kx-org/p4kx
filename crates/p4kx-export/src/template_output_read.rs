// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::CatalogError;
use crate::template_output_plan::TemplateOutputSelection;
use crate::template_projection::TemplateOutputBinding;
use crate::template_reference_index::{TargetSelectionKind, TemplateReferences};
use crate::template_source_members::TemplateMembers;

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    references: &TemplateReferences<'fields, 'scope, 'read, 'source>,
    selected: &[TemplateOutputSelection<'fields, 'scope, 'read, 'source>],
) -> Result<Vec<TemplateOutputBinding>, CatalogError> {
    selected
        .iter()
        .map(|selection| {
            Ok(TemplateOutputBinding {
                caller: members.value_ref(selection.caller)?,
                variable: crate::template_binding_read::property_selected(
                    members,
                    references,
                    &selection.variable,
                )?,
                objective: crate::template_binding_read::objective_selected(
                    members,
                    references,
                    &selection.objective,
                )?,
                selected: matches!(
                    selection.objective.kind(),
                    TargetSelectionKind::Instance { .. }
                ),
            })
        })
        .collect()
}
