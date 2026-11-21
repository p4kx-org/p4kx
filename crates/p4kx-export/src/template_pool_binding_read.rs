// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::Value;

use crate::mission_flow_owner::FlowOwnerKind;
use crate::template_source_members::TemplateMembers;
use crate::template_spawn_plan::TemplateSpawnPlan;
use crate::template_spawn_projection::TemplatePoolBinding;
use crate::template_spawn_read::TemplateSpawns;
use crate::{CatalogError, ExportAccountingError};

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    plan: &TemplateSpawnPlan<'fields, 'scope, 'read, 'source>,
    spawns: &TemplateSpawns<'fields, 'scope, 'read, 'source>,
    owner: FlowOwnerKind,
) -> Result<Vec<TemplatePoolBinding>, CatalogError> {
    let mut selected = BTreeMap::new();
    for declaration in &plan.root {
        let (Value::String(name) | Value::Enum(name)) = declaration.variable.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        selected.entry(*name).or_insert(declaration);
    }
    let mut rows = Vec::new();
    for declaration in selected.into_values() {
        let Some(registered) = spawns.registration(declaration.value)? else {
            continue;
        };
        if registered.empty {
            continue;
        }
        rows.push(TemplatePoolBinding {
            property: members.value_ref(declaration.property)?,
            variable: members.field_ref(declaration.variable)?,
            token: crate::template_declaration_binding::field(
                members,
                Some(declaration.property),
                "extendedTextToken",
            )?,
            registration: members.value_ref(declaration.value)?,
            level: owner.spawn_level(),
        });
    }
    Ok(rows)
}
