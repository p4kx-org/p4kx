// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::Guid;

use super::ReducedError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::variant_target::VariantTarget;
use crate::{CatalogError, ExportAccountingError};

#[derive(Default)]
pub(super) struct RecordTargets {
    records: HashMap<u32, Guid>,
}

impl RecordTargets {
    pub(super) fn insert(
        &mut self,
        path: &'static str,
        header: &CanonicalRecordHeader<'_>,
    ) -> Result<(), ReducedError> {
        let index = header
            .record_index()
            .value()
            .copied()
            .ok_or(ReducedError::IncompleteScope(path))?;
        let guid = header
            .guid()
            .value()
            .copied()
            .ok_or(ReducedError::IncompleteScope(path))?;
        self.records
            .try_reserve(1)
            .map_err(CatalogError::SourceUseAllocation)?;
        if self.records.insert(index, guid).is_some() {
            return Err(CatalogError::from(ExportAccountingError::ConflictingSourceDatum).into());
        }
        Ok(())
    }

    pub(super) fn verify(
        &self,
        path: &'static str,
        index: u32,
        guid: Guid,
    ) -> Result<(), ReducedError> {
        if self.records.get(&index) != Some(&guid) {
            return Err(ReducedError::UnresolvedRecordLink { path, index });
        }
        Ok(())
    }

    pub(super) fn verify_variant(
        &self,
        path: &'static str,
        variant: Option<&VariantTarget>,
    ) -> Result<(), ReducedError> {
        if let Some(variant) = variant {
            let index = variant
                .target_record_index()
                .value()
                .copied()
                .ok_or(ReducedError::IncompleteScope(path))?;
            let guid = variant
                .guid()
                .value()
                .copied()
                .ok_or(ReducedError::IncompleteScope(path))?;
            self.verify(path, index, guid)?;
        }
        Ok(())
    }
}
