// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, SourcedValue};

use super::BlueprintRewardSource;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::guid_text::CatalogGuid;
use crate::resource_number::ResourceNumber;
use crate::source_value_catalog::{SourceDataType, SourceRecordReference};

impl<'source> BlueprintRewardSource<'source> {
    pub(crate) fn number_position(&self) -> Result<Option<(u32, u64)>, crate::CatalogError> {
        self.chance_ordinal
            .map(|ordinal| {
                self.source
                    .record_index
                    .value()
                    .copied()
                    .map(|index| (index, ordinal))
                    .ok_or_else(|| crate::ExportAccountingError::MissingFieldInspection.into())
            })
            .transpose()
    }

    /// Borrows the physical original owner header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }
    /// Borrows the actual original reference occurrence ordinal within its owner.
    #[must_use]
    pub const fn source_ordinal(&self) -> &SourcedValue<DecimalU64> {
        &self.source_ordinal
    }
    /// Borrows the actual authored pool link and resolved physical target metadata.
    #[must_use]
    pub const fn pool(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.pool
    }
    /// Borrows the original `Reference` or root `WeakPointer` storage declaration.
    #[must_use]
    pub const fn pool_storage(&self) -> &SourcedValue<SourceDataType> {
        &self.pool_storage
    }
    /// Borrows the original nearest enclosing identity selected by the reward rule.
    #[must_use]
    pub const fn contract_id(&self) -> &SourcedValue<CatalogGuid> {
        &self.contract_id
    }
    /// Borrows the matching enclosing debug name with its selection evidence.
    #[must_use]
    pub const fn contract_name(&self) -> &SourcedValue<&'source str> {
        &self.contract_name
    }
    /// Borrows the original local chance without clamping or numeric narrowing.
    #[must_use]
    pub const fn chance(&self) -> &SourcedValue<ResourceNumber> {
        &self.chance
    }
}
