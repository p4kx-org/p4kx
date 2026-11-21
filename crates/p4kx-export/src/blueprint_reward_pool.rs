// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue, SourcedValue};

use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::source_value_catalog::{
    SourceDataType, SourcePayload, SourceRecordReference, read_reference_payload,
};
use crate::{CatalogError, ExportAccountingError};

pub(super) struct PoolLink<'source> {
    pub(super) value: Box<SourceRecordReference<'source>>,
    pub(super) storage: SourcedValue<SourceDataType>,
    pub(super) role: SourceUseRole,
}

pub(super) fn read<'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    fact: &OriginalValueFact<'_, '_, 'source>,
) -> Result<PoolLink<'source>, CatalogError> {
    let storage = fact
        .bind(
            OriginalValuePart::DataType,
            SourceValue::Present(SourceDataType::read(fact.field().storage().data_type())),
            SourceUseRole::Payload,
            true,
        )?
        .into_cell(fields.sources());
    let payload = read_reference_payload(fact.reference_observation()?, fields.sources())?;
    let (value, role) = match payload {
        SourcePayload::RecordReference { value } => (value, SourceUseRole::Payload),
        SourcePayload::WeakReference { value } => (
            Box::new(
                value
                    .into_record()
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?,
            ),
            SourceUseRole::Conversion,
        ),
        _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
    };
    Ok(PoolLink {
        value,
        storage,
        role,
    })
}
