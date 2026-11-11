// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Record, RecordTree, SourceEvidenceError, SourceScope, SourceUses};

use super::OriginalCatalogInput;
use crate::CatalogError;

/// One actual Record/tree read admitted by its original build input.
pub(crate) struct OriginalRecordScope<'read, 'source> {
    record: &'read Record<'source>,
    tree: &'read RecordTree<'source>,
    input: &'read OriginalCatalogInput<'read, 'read>,
}

impl<'read, 'source> OriginalRecordScope<'read, 'source> {
    pub(super) fn new(
        input: &'read OriginalCatalogInput<'read, 'read>,
        record: &'read Record<'source>,
        tree: &'read RecordTree<'source>,
    ) -> Result<Self, CatalogError> {
        input.record_context(record)?;
        if !tree.is_from(record) {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(Self {
            record,
            tree,
            input,
        })
    }

    pub(crate) const fn record(&self) -> &'read Record<'source> {
        self.record
    }

    pub(crate) const fn tree(&self) -> &'read RecordTree<'source> {
        self.tree
    }

    pub(crate) const fn sources(&self) -> &'read SourceUses {
        &self.input.origins().uses
    }

    pub(crate) const fn dataforge(&self) -> &'read SourceScope {
        &self.input.origins().dataforge
    }

    pub(crate) fn struct_name(&self, index: u16) -> Option<&str> {
        self.input.struct_name(index)
    }
}
