// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::guid_text::CatalogGuid;
use crate::{CatalogError, ExportAccountingError};

/// Compact join keys retained from the same admitted original Header.
#[derive(Debug)]
pub(crate) struct ShipIdentity<'source> {
    pub(crate) guid: CatalogGuid,
    pub(crate) class_name: &'source str,
    pub(crate) record_path: &'source str,
}

impl<'source> ShipIdentity<'source> {
    pub(crate) fn read(header: &CanonicalRecordHeader<'source>) -> Result<Self, CatalogError> {
        Ok(Self {
            guid: header
                .guid
                .value()
                .copied()
                .ok_or(ExportAccountingError::MissingFieldInspection)?,
            class_name: header
                .class_name
                .value()
                .copied()
                .ok_or(ExportAccountingError::MissingFieldInspection)?,
            record_path: header
                .record_path
                .value()
                .copied()
                .ok_or(ExportAccountingError::MissingFieldInspection)?,
        })
    }
}
