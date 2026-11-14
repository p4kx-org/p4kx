// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use crate::catalog_read::original::OriginalValueEdge;
use crate::mission_flow_owner::{FlowOwnerKind, MissionFlowOwnerRef};
use crate::template_configuration::{FlowSelection, MissionFlowConfiguration, MissionFlowRef};
use crate::template_meaning_plan::SelectedTemplateField;
use crate::template_plan::TemplatePlan;
use crate::template_source_members::TemplateMembers;
use crate::template_source_refs::{DeclarationBinding, SourceFieldRef};
use crate::{CatalogError, ExportAccountingError};

pub(crate) fn read<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    plan: &TemplatePlan<'_, 'scope, 'read, 'source>,
    owner_kind: FlowOwnerKind,
) -> Result<(Vec<MissionFlowConfiguration>, Vec<MissionFlowRef>), CatalogError> {
    let owner = MissionFlowOwnerRef::read(members.fields(), owner_kind)?;
    let mut rows = Vec::new();
    let mut links = Vec::new();
    for (index, declaration) in plan.flow.declarations.iter().enumerate() {
        let declaration_index = declaration
            .declaration
            .map(|fact| match fact.edge() {
                OriginalValueEdge::Field { ordinal } => {
                    Ok(p4kx_extraction::DecimalU64::new(ordinal))
                }
                OriginalValueEdge::Element { .. } => {
                    Err(ExportAccountingError::ConflictingSourceDatum)
                }
            })
            .transpose()?;
        let binding = match declaration.declaration {
            Some(fact) => DeclarationBinding::Present {
                field: members.field_ref(fact)?,
            },
            None => DeclarationBinding::Absent {
                origin: members.fields().absent(None)?,
            },
        };
        let selection = match (index, declaration.declaration, declaration.selected) {
            (_, None, _) => FlowSelection::Absent,
            (0, _, Some(fact)) => FlowSelection::Selected {
                value: members.value_ref(fact)?,
            },
            (0, _, None) => FlowSelection::NoStruct,
            _ => FlowSelection::ShadowedDeclaration,
        };
        rows.push(MissionFlowConfiguration {
            owner: owner.clone(),
            declaration_index,
            declaration: binding,
            selection,
            triggers: declaration
                .triggers
                .iter()
                .map(|fact| members.value_ref(fact))
                .collect::<Result<_, _>>()?,
            referenced_fields: referenced(members, declaration.declaration, &plan.flow_targets)?,
        });
        links.push(MissionFlowRef {
            owner: owner.clone(),
            declaration_index,
        });
    }
    Ok((rows, links))
}

fn referenced<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    declaration: Option<&crate::catalog_read::original::OriginalValueFact<'scope, 'read, 'source>>,
    targets: &[SelectedTemplateField<'_, 'scope, 'read, 'source>],
) -> Result<Vec<SourceFieldRef>, CatalogError> {
    let Some(declaration) = declaration else {
        return Ok(Vec::new());
    };
    let span = members.fields().span(declaration)?;
    let mut selected = BTreeMap::new();
    for target in targets {
        if span.contains(
            &usize::try_from(target.caller).map_err(|_| ExportAccountingError::CounterOverflow)?,
        ) {
            let OriginalValueEdge::Field { ordinal } = target.field.edge() else {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            };
            selected.insert((target.owner, ordinal), target.field);
        }
    }
    selected
        .into_values()
        .map(|fact| members.field_ref(fact))
        .collect()
}
