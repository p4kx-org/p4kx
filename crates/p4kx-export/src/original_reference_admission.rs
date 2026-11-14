// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, Reference, ReferenceOrigin, SourceEvidenceError, Value};

use super::OriginalValueFact;
use crate::{CatalogError, ExportAccountingError};

pub(super) struct OriginalReferenceAdmission<'read, 'source> {
    pub(super) reference: &'read Reference<'source>,
    pub(super) origin: ReferenceOrigin<'read, 'source>,
    pub(super) storage: DataType,
}

impl<'read, 'source> OriginalValueFact<'_, 'read, 'source> {
    pub(super) fn admit_reference(
        &self,
    ) -> Result<OriginalReferenceAdmission<'read, 'source>, CatalogError> {
        let Value::Reference(reference) = self.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let storage = self.field().storage().data_type();
        if !matches!(storage, DataType::Reference | DataType::WeakPointer) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let origin = reference.original_target();
        if !origin.shares_source_with(self.scope().record()) {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(OriginalReferenceAdmission {
            reference,
            origin,
            storage,
        })
    }
}
