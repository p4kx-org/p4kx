// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourceUses};

use crate::catalog_read::original::{OriginalReferenceObservation, OriginalTargetShape};
use crate::definition_reference::DefinitionStatus;
use crate::guid_text::CatalogGuid;
use crate::{CatalogError, ExportAccountingError};

use super::reference::{SourceRecordReference, SourceWeakReference, SourceWeakTarget};
use super::value::SourcePayload;

pub(super) fn read<'source>(
    observed: OriginalReferenceObservation<'_, '_, '_, 'source>,
    sources: &SourceUses,
) -> Result<SourcePayload<'source>, CatalogError> {
    let storage = observed.caller().field().storage().data_type();
    let captured = observed.capture()?;
    let guid = captured.guid.map(CatalogGuid)?.into_cell(sources);
    let target_record_index = captured.record_index.into_cell(sources);
    match (storage, captured.shape) {
        (DataType::Reference, OriginalTargetShape::Record { source_type }) => {
            Ok(SourcePayload::RecordReference {
                value: SourceRecordReference {
                    target_record_index,
                    guid,
                    record_path: captured.record_path.into_cell(sources),
                    target_type: source_type.into_cell(sources),
                    status: DefinitionStatus::Present,
                },
            })
        }
        (DataType::WeakPointer, shape) => {
            let target = match shape {
                OriginalTargetShape::Record { source_type } => SourceWeakTarget::Record {
                    source_type: source_type.into_cell(sources),
                },
                OriginalTargetShape::Instance {
                    source_type,
                    location,
                } => SourceWeakTarget::Instance {
                    source_type: source_type.into_cell(sources),
                    location: location.into_cell(sources),
                },
            };
            Ok(SourcePayload::WeakReference {
                value: SourceWeakReference {
                    target_record_index,
                    record_guid: guid,
                    record_path: captured.record_path.into_cell(sources),
                    target,
                },
            })
        }
        _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
    }
}

#[cfg(test)]
#[path = "source_reference_borrow_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "source_weak_location_fixture.rs"]
mod weak_location_fixture;
#[cfg(test)]
#[path = "source_weak_location_tests.rs"]
mod weak_location_tests;
