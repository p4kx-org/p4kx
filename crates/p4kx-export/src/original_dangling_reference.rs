// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, Guid, SourceUseRole, SourceValue, Value, ValueError};

use super::{OriginalDatum, OriginalErrorParameter, OriginalValueFact, OriginalValuePart};
use crate::{CatalogError, ExportAccountingError};

impl OriginalValueFact<'_, '_, '_> {
    /// Captures only the authored GUID of the canonical failed Reference lookup.
    pub(crate) fn dangling_guid(
        &self,
        role: SourceUseRole,
    ) -> Result<Option<OriginalDatum<Guid>>, CatalogError> {
        let Value::Error(ValueError::DanglingReference { guid }) = self.value() else {
            return Ok(None);
        };
        if self.field().storage().data_type() != DataType::Reference {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(Some(self.bind(
            OriginalValuePart::Error(OriginalErrorParameter::Guid),
            SourceValue::Present(*guid),
            role,
            true,
        )?))
    }
}
