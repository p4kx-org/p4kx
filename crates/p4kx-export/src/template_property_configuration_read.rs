// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;

use crate::catalog_read::original::{OriginalValueEdge, OriginalValueFact};
use crate::mission_flow_owner::{FlowOwnerKind, MissionFlowOwnerRef};
use crate::template_configuration::{
    MissionPropertyConfiguration, MissionPropertyRef, PropertySelection,
};
use crate::template_source_members::TemplateMembers;
use crate::template_source_refs::DeclarationBinding;
use crate::{CatalogError, ExportAccountingError};

pub(crate) fn read<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    objectives: &[crate::template_objective_plan::TemplateObjectiveSelection<
        '_,
        'scope,
        'read,
        'source,
    >],
    kind: FlowOwnerKind,
) -> Result<(Vec<MissionPropertyConfiguration>, Vec<MissionPropertyRef>), CatalogError> {
    let mut rows = Vec::new();
    let physical = MissionFlowOwnerRef::read(members.fields(), kind)?;
    declarations(members, &physical, None, kind.properties(), true, &mut rows)?;
    for objective in objectives {
        declarations(
            members,
            &physical,
            Some(objective.fact),
            "properties",
            objective.selected,
            &mut rows,
        )?;
    }
    let links = rows
        .iter()
        .map(|row| MissionPropertyRef {
            owner: row.owner.clone(),
            owner_objective: row.owner_objective,
            declaration_index: row.declaration_index,
        })
        .collect();
    Ok((rows, links))
}

fn declarations<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    physical: &MissionFlowOwnerRef,
    owner: Option<&OriginalValueFact<'scope, 'read, 'source>>,
    name: &str,
    selected_owner: bool,
    rows: &mut Vec<MissionPropertyConfiguration>,
) -> Result<(), CatalogError> {
    let fields = members.fields();
    let declarations = fields.declarations(owner, name)?;
    if declarations.is_empty() {
        rows.push(MissionPropertyConfiguration {
            owner: physical.clone(),
            owner_objective: owner.map(|fact| members.value_ref(fact)).transpose()?,
            declaration_index: None,
            declaration: DeclarationBinding::Absent {
                origin: fields.absent(owner)?,
            },
            selection: PropertySelection::Absent,
            properties: Vec::new(),
        });
        return Ok(());
    }
    for (index, declaration) in declarations.into_iter().enumerate() {
        let OriginalValueEdge::Field { ordinal } = declaration.edge() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let values = if owner.is_some()
            && matches!(declaration.value(), p4kx_extraction::Value::Struct(_))
        {
            vec![declaration]
        } else {
            fields.structs(declaration)?
        };
        let properties = values
            .into_iter()
            .map(|fact| members.value_ref(fact))
            .collect::<Result<Vec<_>, _>>()?;
        let selection = if !selected_owner {
            PropertySelection::ShadowedObjective
        } else if index != 0 {
            PropertySelection::ShadowedDeclaration
        } else if properties.is_empty() {
            PropertySelection::NoStruct
        } else {
            PropertySelection::Selected
        };
        rows.push(MissionPropertyConfiguration {
            owner: physical.clone(),
            owner_objective: owner.map(|fact| members.value_ref(fact)).transpose()?,
            declaration_index: Some(DecimalU64::new(ordinal)),
            declaration: DeclarationBinding::Present {
                field: members.field_ref(declaration)?,
            },
            selection,
            properties,
        });
    }
    Ok(())
}
