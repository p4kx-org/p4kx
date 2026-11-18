// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Record, SourceValue, SourcedValue};

use super::Capture;
use crate::record_header_field::RecordHeaderField;
use crate::{CatalogError, ExportAccountingError};

impl Capture<'_> {
    pub(in crate::loadout) fn source_record_index(
        &self,
        record: &Record<'_>,
    ) -> Result<SourcedValue<u32>, CatalogError> {
        #[cfg(test)]
        if self.input.is_none() {
            return Ok(crate::contract_loadout::fixture_record_index(
                record.index(),
            ));
        }
        let input = self
            .input
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        input.record_context(record)?.header_selector(
            RecordHeaderField::RecordIndex,
            SourceValue::Present(record.index()),
        )
    }
}
