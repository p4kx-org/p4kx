// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, SourceValue, SourcedValue, Value};

use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::guid_text::CatalogGuid;
use crate::record_fields::text_value;
use crate::{CatalogError, ExportAccountingError};

pub(super) struct Identity<'source> {
    pub(super) id: SourcedValue<CatalogGuid>,
    pub(super) name: SourcedValue<&'source str>,
}

pub(super) fn read<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    owner: Option<&OriginalValueFact<'scope, 'read, 'source>>,
) -> Result<Identity<'source>, CatalogError> {
    let owners = ancestors(fields, owner)?;
    let mut id_uses = Vec::new();
    let mut id_complete = true;
    for owner in &owners {
        fields.check_cancel()?;
        let fact = inspected(fields, *owner, "id", &mut id_uses, &mut id_complete)?;
        if let Some(Value::Guid(guid)) = fact.map(OriginalValueFact::value) {
            let id = fields.sources().value(
                SourceValue::Present(CatalogGuid(*guid)),
                &id_uses,
                id_complete,
            );
            let mut name_complete = id_complete;
            let name = inspected(
                fields,
                *owner,
                "debugName",
                &mut id_uses,
                &mut name_complete,
            )?
            .and_then(|fact| text_value(fact.value()));
            return Ok(Identity {
                id,
                name: fields.sources().value(
                    name.map_or(SourceValue::Absent, SourceValue::Present),
                    &id_uses,
                    name_complete,
                ),
            });
        }
    }
    let id = fields
        .sources()
        .value(SourceValue::Absent, &id_uses, id_complete);
    let mut name_complete = id_complete;
    let mut name = None;
    for owner in owners {
        fields.check_cancel()?;
        name = inspected(fields, owner, "debugName", &mut id_uses, &mut name_complete)?
            .and_then(|fact| text_value(fact.value()));
        if name.is_some() {
            break;
        }
    }
    Ok(Identity {
        id,
        name: fields.sources().value(
            name.map_or(SourceValue::Absent, SourceValue::Present),
            &id_uses,
            name_complete,
        ),
    })
}

fn inspected<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    owner: Option<&OriginalValueFact<'scope, 'read, 'source>>,
    name: &str,
    inputs: &mut Vec<SourceUse>,
    complete: &mut bool,
) -> Result<Option<&'fields OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
    let Some((_, fact)) = fields.field(owner, name)? else {
        inputs.push(fields.survey(owner)?);
        return Ok(None);
    };
    inputs.push(fact.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?);
    let readable = !matches!(fact.value(), Value::Error(_));
    *complete &= readable;
    let part = if matches!(fact.value(), Value::Struct(_)) {
        OriginalValuePart::ConcreteType
    } else {
        OriginalValuePart::Value
    };
    inputs.push(fact.observe(
        part,
        if readable {
            SourceUseRole::Selector
        } else {
            SourceUseRole::Attempted
        },
    )?);
    Ok(Some(fact))
}

fn ancestors<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    mut owner: Option<&'fields OriginalValueFact<'scope, 'read, 'source>>,
) -> Result<Vec<Option<&'fields OriginalValueFact<'scope, 'read, 'source>>>, CatalogError> {
    let mut owners = Vec::new();
    while let Some(current) = owner {
        fields.check_cancel()?;
        if matches!(current.value(), Value::Struct(_)) {
            owners.push(Some(current));
        }
        owner = current
            .parent_ordinal()
            .map(|ordinal| {
                usize::try_from(ordinal)
                    .ok()
                    .and_then(|index| fields.facts().get(index))
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)
            })
            .transpose()?;
    }
    owners.push(None);
    Ok(owners)
}
