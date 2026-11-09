// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue, SourceValueState, SourcedValue};

use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::commodity_inputs::CommodityInputs;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};

/// Captures the exact observed field contribution without reconstructing origin references.
pub(crate) fn field<T>(
    inputs: &mut CommodityInputs<'_>,
    record_index: u32,
    path: &str,
    value: &SourcedValue<T>,
) -> Result<(), CatalogError> {
    let role = match value.state() {
        SourceValueState::Present | SourceValueState::Null | SourceValueState::Invalid => {
            SourceUseRole::Payload
        }
        SourceValueState::Absent | SourceValueState::Unsupported => SourceUseRole::Selector,
        _ => SourceUseRole::Attempted,
    };
    inputs.at(record_index, path, role)?;
    inputs.require_complete(value.origin().source_set_complete());
    Ok(())
}

/// Includes only the retained reference GUID and independently observed target headers.
pub(crate) fn reference(
    inputs: &mut CommodityInputs<'_>,
    record_index: u32,
    path: &str,
    reference: &SourcedValue<CatalogRecordReference>,
) -> Result<(), CatalogError> {
    field(inputs, record_index, path, reference)?;
    let Some(reference) = reference.value() else {
        return Ok(());
    };
    inputs.require_complete(reference.guid.origin().source_set_complete());
    let target = reference.target_record_index.value().copied();
    for (header, complete) in [
        (
            RecordHeaderInput::Field(RecordHeaderField::RecordIndex),
            reference.target_record_index.origin().source_set_complete(),
        ),
        (
            RecordHeaderInput::NormalizedPath,
            reference.record_path.origin().source_set_complete(),
        ),
        (
            RecordHeaderInput::Field(RecordHeaderField::SourceType),
            reference.target_type.origin().source_set_complete(),
        ),
    ] {
        if let Some(target) = target {
            inputs.header_input(target, header, SourceUseRole::Selector)?;
        } else {
            inputs.at(record_index, path, SourceUseRole::Selector)?;
        }
        inputs.require_complete(complete);
    }
    Ok(())
}

/// Returns the source state when no typed payload can be retained.
pub(crate) const fn without_payload<T>(state: SourceValueState) -> Option<SourceValue<T>> {
    match state {
        SourceValueState::Present => None,
        SourceValueState::Absent => Some(SourceValue::Absent),
        SourceValueState::Null => Some(SourceValue::Null),
        SourceValueState::Unreadable => Some(SourceValue::Unreadable),
        SourceValueState::Invalid => Some(SourceValue::Invalid),
        _ => Some(SourceValue::Unsupported),
    }
}
