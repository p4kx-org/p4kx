// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, Guid, SourceUse, SourceUseRole, SourceValue, Value, ValueError};

use crate::build::original_input::{
    append_record_guid_inventory, append_record_instance_inventory,
};

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

    /// Observes a resolved exclusion for an original link without a Record target.
    pub(crate) fn observe_unresolved_link(
        &self,
        uses: &mut Vec<SourceUse>,
    ) -> Result<bool, CatalogError> {
        if matches!(self.value(), Value::Error(ValueError::OutsideRecord { .. })) {
            if self.field().storage().data_type() != DataType::WeakPointer {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            for part in [
                OriginalValuePart::DataType,
                OriginalValuePart::Error(OriginalErrorParameter::StructIndex),
                OriginalValuePart::Error(OriginalErrorParameter::InstanceIndex),
            ] {
                uses.push(self.observe(part, SourceUseRole::Selector)?);
            }
            append_record_instance_inventory(self.scope().input(), uses)?;
            return Ok(true);
        }
        self.observe_dangling_reference(uses)
    }

    /// Observes an original Reference whose authored GUID has no Record target.
    pub(crate) fn observe_dangling_reference(
        &self,
        uses: &mut Vec<SourceUse>,
    ) -> Result<bool, CatalogError> {
        let Some(guid) = self.dangling_guid(SourceUseRole::Selector)? else {
            return Ok(false);
        };
        let (_, inputs, complete) = guid.into_parts();
        if !complete {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        uses.extend(inputs);
        uses.push(self.observe(OriginalValuePart::DataType, SourceUseRole::Selector)?);
        append_record_guid_inventory(self.scope().input(), uses)?;
        Ok(true)
    }
}
