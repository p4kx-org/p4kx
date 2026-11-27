// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::EntityNode;
use p4kx_extraction::{
    Guid, SourceUseRole, SourceValue, SourcedValue, StructValue, Value, ValueError,
};

use super::Capture;
use crate::catalog_read::original::{
    OriginalErrorParameter, OriginalFlowFields, OriginalValueFact, OriginalValuePart,
};
use crate::guid_text::CatalogGuid;
use crate::{CatalogError, ExportAccountingError};

impl Capture<'_> {
    pub(super) fn store_cargo_storage(
        &self,
        fields: &OriginalFlowFields<'_, '_, '_>,
        component: Option<&StructValue<'_>>,
        value: Option<&Value<'_>>,
        guid: Option<Guid>,
    ) -> Result<(), CatalogError> {
        let owner = component
            .map(|component| owner(fields, component))
            .transpose()?;
        let mut selection = fields.component_selection(owner.map(OriginalValueFact::value))?;
        let state = if let Some(value) = value {
            let (_, actual) = fields
                .field(owner, "containerParams")?
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            if owner.is_none() || !std::ptr::addr_eq(actual.value(), value) {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            let (state, part, role) = storage_value(value);
            selection.complete &= !matches!(state, SourceValue::Unreadable);
            selection
                .uses
                .push(actual.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?);
            selection.uses.push(actual.observe(part, role)?);
            state
        } else if let Some(owner) = owner {
            if fields.field(Some(owner), "containerParams")?.is_some() {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            selection.uses.push(fields.survey(Some(owner))?);
            SourceValue::Absent
        } else if selection.complete {
            SourceValue::Absent
        } else {
            SourceValue::Unreadable
        };
        let proof = fields
            .sources()
            .value(state, &selection.uses, selection.complete);
        if proof.value().map(|guid| guid.0) != guid {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let mut state = self.lock()?;
        let index = fields.record_index();
        if let Some(prior) = state.cargo_storage_origins.get(&index) {
            if prior != &proof {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
        } else {
            state.cargo_storage_origins.insert(index, proof);
        }
        drop(state);
        Ok(())
    }

    pub(crate) fn cargo_storage(
        &self,
        node: &EntityNode,
        record: u32,
    ) -> Result<SourcedValue<CatalogGuid>, CatalogError> {
        let proof = self
            .lock()?
            .cargo_storage_origins
            .get(&record)
            .cloned()
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        if proof.value().map(|value| value.0) != node.inventory_container() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(proof)
    }
}

fn storage_value(
    value: &Value<'_>,
) -> (SourceValue<CatalogGuid>, OriginalValuePart, SourceUseRole) {
    let (state, part) = match value {
        Value::Reference(link) => (
            SourceValue::Present(CatalogGuid(link.guid())),
            OriginalValuePart::Value,
        ),
        Value::Error(ValueError::DanglingReference { guid }) => (
            SourceValue::Present(CatalogGuid(*guid)),
            OriginalValuePart::Error(OriginalErrorParameter::Guid),
        ),
        Value::Null => (SourceValue::Null, OriginalValuePart::Value),
        Value::Error(_) => {
            return (
                SourceValue::Unreadable,
                OriginalValuePart::Value,
                SourceUseRole::Attempted,
            );
        }
        _ => (SourceValue::Unsupported, OriginalValuePart::Value),
    };
    (state, part, SourceUseRole::Selector)
}

fn owner<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    component: &StructValue<'_>,
) -> Result<&'fields OriginalValueFact<'scope, 'read, 'source>, CatalogError> {
    let fact = fields
        .facts()
        .iter()
        .find(|fact| {
            matches!(fact.value(), Value::Struct(original)
            if std::ptr::addr_eq(original, component))
        })
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    Ok(fact)
}
