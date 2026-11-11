// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, Value};

use crate::catalog_projection::CatalogProjection;
use crate::mission_flow_owner::FlowOwnerKind;
use crate::source_value_catalog::SourceValueRef;
use crate::template_context_value::TemplateLocalization;
use crate::template_objective_plan::TemplateObjectiveSelection;
use crate::template_projection::TemplateObjectiveProjection;
use crate::template_source_members::TemplateMembers;
use crate::{CatalogError, ExportAccountingError};

pub(crate) fn objectives<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    plan: &[TemplateObjectiveSelection<'_, 'scope, 'read, 'source>],
) -> Result<Vec<TemplateObjectiveProjection>, CatalogError> {
    plan.iter()
        .filter(|selection| selection.selected)
        .map(|selection| {
            let (_, id) = members
                .fields()
                .field(Some(selection.fact), "id")?
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            Ok(TemplateObjectiveProjection {
                source: members.value_ref(selection.fact)?,
                parent: selection
                    .parent
                    .map(|parent| members.value_ref(parent))
                    .transpose()?,
                id: members.field_ref(id)?,
            })
        })
        .collect()
}

pub(crate) fn title(
    members: &TemplateMembers<'_, '_, '_, '_>,
) -> Result<Option<SourceValueRef>, CatalogError> {
    let fields = members.fields();
    let Some((_, display)) = fields.field(None, "contractDisplayInfo")? else {
        return Ok(None);
    };
    let Some(display) = fields.first_struct(display)? else {
        return Ok(None);
    };
    let Some((_, strings)) = fields.field(Some(display), "displayString")? else {
        return Ok(None);
    };
    let first = match strings.value() {
        Value::Struct(_) => Some(strings),
        Value::Array(_) => fields.elements(strings)?.into_iter().next(),
        _ => None,
    };
    let Some(first) = first else {
        return Ok(None);
    };
    let key = match first.value() {
        Value::Struct(_) => fields.field(Some(first), "value")?.map(|(_, fact)| fact),
        _ => Some(first),
    };
    key.map(|key| members.value_ref(key)).transpose()
}

/// Projects actual locale datums and selected permissive text lookups into the `GameText` pool.
pub(crate) fn localizations<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    objectives: &[TemplateObjectiveSelection<'_, 'scope, 'read, 'source>],
    properties: &[crate::template_property_plan::SelectedProperty<'_, 'scope, 'read, 'source>],
    owner: FlowOwnerKind,
) -> Result<Vec<TemplateLocalization>, CatalogError> {
    let title = match owner {
        FlowOwnerKind::ContractTemplate => title(members)?,
        FlowOwnerKind::MissionBrokerEntry => None,
    };
    let selected = crate::template_localization_plan::selected(
        members.fields(),
        objectives,
        properties,
        owner,
    )?;
    let mut rows = Vec::new();
    for fact in members.values() {
        members.fields().check_cancel()?;
        let source = members.value_ref(fact)?;
        if title.is_some_and(|title| title.value_index == source.value_index)
            || fact.field().storage().data_type() == DataType::Locale
            || selected.contains(&fact.ordinal())
        {
            rows.push(crate::template_localization_read::read(
                members, projection, fact,
            )?);
        }
    }
    Ok(rows)
}
