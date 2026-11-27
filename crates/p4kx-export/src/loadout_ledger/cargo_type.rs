// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::btree_map::Entry;

use p4kx_assembly::ItemClassification;
use p4kx_extraction::{FieldOrigin, SourceValue, StructValue, Value};

use super::Capture;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::{CatalogError, ExportAccountingError};

impl Capture<'_> {
    pub(super) fn store_cargo_type(
        &self,
        fields: &OriginalFlowFields<'_, '_, '_>,
        component: Option<(usize, &StructValue<'_>)>,
        classification: &ItemClassification,
    ) -> Result<(), CatalogError> {
        let selected = component
            .map(|(ordinal, original)| selected_component(fields, ordinal, original))
            .transpose()?;
        let mut input = fields.component_selection(selected.map(OriginalValueFact::value))?;
        if let Some(selected) = selected {
            let mut retain = |used| {
                input.uses.push(used);
                Ok(())
            };
            if let Some(definition) =
                fields.observe_field_selection(selected, "AttachDef", &mut retain)?
                && matches!(definition.value(), Value::Struct(_))
            {
                let _ = fields.observe_field_selection(definition, "Type", &mut retain)?;
            }
        }
        let complete = input.complete
            && matches!(
                classification.item_type(),
                SourceValue::Present(_) | SourceValue::Absent | SourceValue::Null
            );
        let origin = fields.sources().origin(&input.uses, complete);
        let mut state = self.lock()?;
        match state.cargo_type_origins.entry(fields.record_index()) {
            Entry::Vacant(entry) => {
                entry.insert(origin);
            }
            Entry::Occupied(entry) if entry.get() == &origin => {}
            Entry::Occupied(_) => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
        drop(state);
        Ok(())
    }

    pub(crate) fn cargo_type_origin(&self, record_index: u32) -> Result<FieldOrigin, CatalogError> {
        self.lock()?
            .cargo_type_origins
            .get(&record_index)
            .cloned()
            .ok_or_else(|| ExportAccountingError::UnobservedFailure.into())
    }
}

fn selected_component<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    ordinal: usize,
    original: &StructValue<'_>,
) -> Result<&'fields OriginalValueFact<'scope, 'read, 'source>, CatalogError> {
    let Some(Value::Array(components)) = fields.root().field("Components") else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let value = components
        .get(ordinal)
        .filter(|value| {
            matches!(value, Value::Struct(component) if std::ptr::addr_eq(component, original))
        })
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    fields.fact(value)
}
