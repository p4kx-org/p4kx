// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceLocation, StructValue};

use super::OriginalFlowFields;
use crate::catalog_read::original::{OriginalRecordScope, OriginalValueFact};
use crate::{CatalogError, ExportAccountingError};

/// A fixed requested name absent from one complete original Struct declaration survey.
pub(crate) struct OriginalMissingField<'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    parent: Option<&'fields OriginalValueFact<'scope, 'read, 'source>>,
    source: &'fields StructValue<'source>,
    path: String,
}

impl<'scope, 'read, 'source> OriginalFlowFields<'scope, 'read, 'source> {
    pub(crate) fn missing_field<'fields>(
        &'fields self,
        source: &'fields StructValue<'source>,
        name: &'static str,
    ) -> Result<OriginalMissingField<'fields, 'scope, 'read, 'source>, CatalogError> {
        let parent = self.owner_of(source)?;
        if self.field(parent, name)?.is_some() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let mut path = parent
            .map(|parent| self.logical_path(parent))
            .transpose()?
            .unwrap_or_default();
        if !path.is_empty() {
            path.push('.');
        }
        path.push_str(name);
        Ok(OriginalMissingField {
            fields: self,
            parent,
            source,
            path,
        })
    }
}

impl<'fields, 'scope, 'read, 'source> OriginalMissingField<'fields, 'scope, 'read, 'source> {
    pub(crate) const fn scope(&self) -> &'scope OriginalRecordScope<'read, 'source> {
        self.fields.scope
    }

    pub(crate) const fn parent(
        &self,
    ) -> Option<&'fields OriginalValueFact<'scope, 'read, 'source>> {
        self.parent
    }

    pub(crate) const fn source(&self) -> &'fields StructValue<'source> {
        self.source
    }
    pub(crate) const fn path(&self) -> &str {
        self.path.as_str()
    }

    pub(crate) fn ancestors(
        &self,
    ) -> Result<Vec<&OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
        self.parent
            .map(|parent| self.fields.ancestors(parent))
            .transpose()
            .map(Option::unwrap_or_default)
    }

    pub(crate) fn survey_location(&self) -> Result<SourceLocation, CatalogError> {
        self.fields.survey_location(self.parent)
    }
}
