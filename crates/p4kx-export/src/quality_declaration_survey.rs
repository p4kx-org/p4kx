// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, SourceUse, SourceUseRole, StructValue};

use crate::catalog_read::original::{OriginalFlowFields, OriginalRecordScope};
use crate::{CatalogError, ExportAccountingError};

/// Reuses the admitted actual Structure declaration inventory for nested absent fields.
pub(super) struct Surveys<'scope, 'read, 'source> {
    scope: &'scope OriginalRecordScope<'read, 'source>,
    cancel: Option<&'scope CancelToken>,
    fields: Option<OriginalFlowFields<'scope, 'read, 'source>>,
}

impl<'scope, 'read, 'source> Surveys<'scope, 'read, 'source> {
    pub(super) const fn new(
        scope: &'scope OriginalRecordScope<'read, 'source>,
        cancel: Option<&'scope CancelToken>,
    ) -> Self {
        Self {
            scope,
            cancel,
            fields: None,
        }
    }

    pub(super) fn missing(
        &mut self,
        parent: &StructValue<'source>,
    ) -> Result<SourceUse, CatalogError> {
        if std::ptr::eq(parent, self.scope.tree().root()) {
            return Ok(self.scope.dataforge().at(
                self.scope.root_declaration_location(),
                SourceUseRole::Selector,
            )?);
        }
        if self.fields.is_none() {
            self.fields = Some(OriginalFlowFields::read(self.scope, self.cancel)?);
        }
        let fields = self
            .fields
            .as_ref()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        survey(fields, parent)
    }
}

pub(super) fn survey<'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    parent: &StructValue<'source>,
) -> Result<SourceUse, CatalogError> {
    fields.survey(fields.owner_of(parent)?)
}
