// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceUses, SourceValue, Value};

use crate::catalog_read::original::{
    OriginalReferenceObservation, OriginalValueFact, OriginalValuePart,
};
use crate::guid_text::CatalogGuid;
use crate::resource_number::ResourceNumber;
use crate::{CatalogError, ExportAccountingError};

use super::text::SourceText;
use super::value::{SourceNode, SourcePayload};
use super::{error_read, reference_read};

pub(crate) fn read<'source>(
    fact: &OriginalValueFact<'_, '_, 'source>,
    sources: &SourceUses,
) -> Result<SourceNode<'source>, CatalogError> {
    if matches!(fact.value(), Value::Reference(_)) {
        return read_reference(fact.reference_observation()?, sources);
    }
    let (value, original_error) = match fact.value() {
        Value::Null => (SourceValue::Null, None),
        Value::Error(_) => (
            SourceValue::Unreadable,
            Some(error_read::read(fact, sources)?),
        ),
        value => (SourceValue::Present(payload(fact, value)?), None),
    };
    Ok(SourceNode {
        value: fact
            .bind(
                OriginalValuePart::Value,
                value,
                SourceUseRole::Payload,
                true,
            )?
            .into_cell(sources),
        original_error,
    })
}

pub(crate) fn read_reference<'source>(
    observed: OriginalReferenceObservation<'_, '_, '_, 'source>,
    sources: &SourceUses,
) -> Result<SourceNode<'source>, CatalogError> {
    let fact = observed.caller();
    let payload = reference_read::read(observed, sources)?;
    Ok(SourceNode {
        value: fact
            .bind(
                OriginalValuePart::Value,
                SourceValue::Present(payload),
                SourceUseRole::Payload,
                true,
            )?
            .into_cell(sources),
        original_error: None,
    })
}

fn payload<'source>(
    fact: &OriginalValueFact<'_, '_, 'source>,
    value: &Value<'source>,
) -> Result<SourcePayload<'source>, CatalogError> {
    if let Some(value) = ResourceNumber::read(value) {
        return Ok(SourcePayload::Number { value });
    }
    Ok(match value {
        Value::Bool(value) => SourcePayload::Boolean { value: *value },
        Value::String(value) => SourcePayload::String {
            value: SourceText::read(fact.owner_type(), fact.field().name(), value),
        },
        Value::Locale(value) => SourcePayload::Locale {
            value: SourceText::read(fact.owner_type(), fact.field().name(), value),
        },
        Value::Enum(value) => SourcePayload::Enum { value },
        Value::Guid(value) => SourcePayload::Guid {
            value: CatalogGuid(*value),
        },
        _ => return Err(ExportAccountingError::UnsupportedLootSourceValue.into()),
    })
}
