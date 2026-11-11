// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, SourceValue, SourcedValue, Value};

use crate::CatalogError;
use crate::blueprint_field_source::BlueprintField;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::original::OriginalValuePart;

/// Actual choices that may acquire further contributors after parallel selection.
#[derive(Debug)]
pub(crate) struct BlueprintChoice<T> {
    pub(crate) state: SourceValue<T>,
    pub(crate) used: Vec<SourceUse>,
    pub(crate) complete: bool,
}

impl<T> BlueprintChoice<T> {
    pub(crate) fn cell(self, origins: &CatalogOrigins) -> SourcedValue<T> {
        origins.uses.value(self.state, &self.used, self.complete)
    }
}

pub(super) fn reference(
    field: &BlueprintField<'_, '_, '_, '_>,
) -> Result<BlueprintChoice<crate::guid_text::CatalogGuid>, CatalogError> {
    use crate::blueprint_field_source::normalize;
    let (state, used, complete) = match field.fact() {
        Some(fact) => {
            let resolved = if field.is_blocked() {
                None
            } else {
                crate::record_fields::record_reference(fact.value())
            };
            let state = normalize(fact.value(), resolved);
            let complete = !matches!(fact.value(), Value::Error(_));
            let role = if !complete {
                SourceUseRole::Attempted
            } else if resolved.is_some() {
                SourceUseRole::Payload
            } else {
                SourceUseRole::Selector
            };
            (
                state,
                fact.observe(OriginalValuePart::Value, role)?,
                complete,
            )
        }
        None => (SourceValue::Absent, field.absence_input()?, true),
    };
    Ok(BlueprintChoice {
        state,
        used: vec![used],
        complete,
    })
}
