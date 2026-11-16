// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, Guid, SourceUseRole, SourceValue};

use super::health_reference_data::ReferenceData;
use super::health_reference_observation::ReferenceBody;
use super::health_reference_path::ReferencePathUses;
use super::health_reference_receipt::{ReferenceDatum, ReferenceUse};
use super::{OriginalValueFact, OriginalValuePart};
use crate::ExportAccountingError;
use crate::record_header_field::RecordHeaderField;

/// The admitted payload address of the actual caller's Reference token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ReferenceCaller {
    record: u32,
    ordinal: u64,
}

impl ReferenceCaller {
    pub(super) const fn from_fact(fact: &OriginalValueFact<'_, '_, '_>) -> Self {
        Self {
            record: fact.scope().record().index(),
            ordinal: fact.ordinal(),
        }
    }

    pub(super) fn operand(self, role: SourceUseRole) -> ReferenceUse {
        ReferenceUse::positioned(self.record, self.ordinal, OriginalValuePart::Value, role)
    }
}

/// Original path selectors and caller storage needed to reconstruct a read target receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReferenceRecipe {
    pub(super) path: ReferencePathUses,
    pub(super) owner_type: ReferenceUse,
    pub(super) caller: ReferenceCaller,
    pub(super) storage: DataType,
}

impl ReferenceRecipe {
    pub(super) fn body(self, data: &ReferenceData) -> Result<ReferenceBody, ExportAccountingError> {
        let index = data
            .target_record_index
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let guid = data
            .target_guid
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let path = data
            .target_record_path
            .clone()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let target_type = data
            .target_type
            .clone()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let selector = self.caller.operand(SourceUseRole::Selector);
        Ok(ReferenceBody {
            field: ReferenceDatum::captured(
                SourceValue::Present(data.field.clone()),
                self.path.into_vec(),
            ),
            source_type: ReferenceDatum::captured(
                SourceValue::Present(data.source_type.clone()),
                vec![self.owner_type],
            ),
            target_guid: guid_datum(guid, index, self.storage, self.caller, &selector)?,
            target_record_index: target_value(
                index,
                index,
                RecordHeaderField::RecordIndex,
                SourceUseRole::Selector,
                &selector,
            ),
            target_record_path: target_value(
                path,
                index,
                RecordHeaderField::FileName,
                SourceUseRole::Conversion,
                &selector,
            ),
            target_type: target_value(
                target_type,
                index,
                RecordHeaderField::SourceType,
                SourceUseRole::Selector,
                &selector,
            ),
            target_location: ReferenceDatum::absent(),
            source_pointer: None,
        })
    }
}

pub(super) fn guid_datum(
    guid: Guid,
    index: u32,
    storage: DataType,
    caller: ReferenceCaller,
    selector: &ReferenceUse,
) -> Result<ReferenceDatum<Guid>, ExportAccountingError> {
    let inputs = match storage {
        DataType::Reference => vec![caller.operand(SourceUseRole::Payload)],
        DataType::WeakPointer => vec![
            header(index, RecordHeaderField::Guid, SourceUseRole::Payload),
            selector.clone(),
            caller.operand(SourceUseRole::Conversion),
        ],
        _ => return Err(ExportAccountingError::ConflictingSourceDatum),
    };
    Ok(ReferenceDatum::captured(SourceValue::Present(guid), inputs))
}

fn target_value<T>(
    value: T,
    index: u32,
    field: RecordHeaderField,
    role: SourceUseRole,
    selector: &ReferenceUse,
) -> ReferenceDatum<T> {
    ReferenceDatum::captured(
        SourceValue::Present(value),
        vec![header(index, field, role), selector.clone()],
    )
}

fn header(index: u32, field: RecordHeaderField, role: SourceUseRole) -> ReferenceUse {
    ReferenceUse::header(index, field, role)
}
