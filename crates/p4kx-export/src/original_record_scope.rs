// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Record, RecordTree, SourceEvidenceError, SourceScope, SourceUses};

use super::OriginalCatalogInput;
use crate::CatalogError;

/// One actual Record/tree read admitted by its original build input.
pub(crate) struct OriginalRecordScope<'read, 'source> {
    record: &'read Record<'source>,
    tree: &'read RecordTree<'source>,
    sources: &'read SourceUses,
    dataforge: &'read SourceScope,
}

impl<'read, 'source> OriginalRecordScope<'read, 'source> {
    pub(super) fn new(
        input: &'read OriginalCatalogInput<'_, '_>,
        record: &'read Record<'source>,
        tree: &'read RecordTree<'source>,
    ) -> Result<Self, CatalogError> {
        let context = input.record_context(record)?;
        if !tree.is_from(record) {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(Self {
            record,
            tree,
            sources: context.sources(),
            dataforge: context.dataforge(),
        })
    }

    pub(crate) const fn record(&self) -> &'read Record<'source> {
        self.record
    }

    pub(crate) const fn tree(&self) -> &'read RecordTree<'source> {
        self.tree
    }

    pub(crate) const fn sources(&self) -> &'read SourceUses {
        self.sources
    }

    pub(crate) const fn dataforge(&self) -> &'read SourceScope {
        self.dataforge
    }
}
