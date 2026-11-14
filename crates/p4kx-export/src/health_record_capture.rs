// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, Record};

use super::OriginalDatum;
use super::health_header_receipt::HeaderReadReceipt;
use super::health_text_capture::CapturedText;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;

/// Original header values materialized for a retained failed Record or human failure.
#[derive(Debug, Clone)]
pub(super) struct CapturedRecord {
    pub(super) record_index: OriginalDatum<u32>,
    pub(super) guid: OriginalDatum<Guid>,
    pub(super) record_path: OriginalDatum<CapturedText>,
    pub(super) source_type: OriginalDatum<CapturedText>,
}

impl CapturedRecord {
    pub(super) fn read(
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
    ) -> Result<Self, CatalogError> {
        HeaderReadReceipt::read(input, record)?.record(&input.origins().uses)
    }
}
