// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourceLocation, SourceUse, SourceUseRole, Value};

use super::OriginalFlowFields;
use crate::CatalogError;
use crate::catalog_read::original::{OriginalRecordScope, OriginalValueFact};

impl<'scope, 'read, 'source> OriginalFlowFields<'scope, 'read, 'source> {
    /// Locates the complete declaration list of one materialized original Struct.
    pub(crate) fn survey_location(
        &self,
        parent: Option<&OriginalValueFact<'scope, 'read, 'source>>,
    ) -> Result<SourceLocation, CatalogError> {
        self.check_cancel()?;
        Ok(match parent {
            Some(fact) => {
                self.span(fact)?;
                if !matches!(fact.value(), Value::Struct(_)) {
                    return Err(crate::ExportAccountingError::ConflictingSourceDatum.into());
                }
                SourceLocation::Subresource {
                    namespace: "dataforgeValue".to_owned(),
                    indices: vec![u64::from(self.record_index()), fact.ordinal()],
                    field: "$fields".to_owned(),
                }
            }
            None => self.scope.root_declaration_location(),
        })
    }

    pub(crate) fn survey(
        &self,
        parent: Option<&OriginalValueFact<'scope, 'read, 'source>>,
    ) -> Result<SourceUse, CatalogError> {
        Ok(self
            .scope
            .dataforge()
            .at(self.survey_location(parent)?, SourceUseRole::Selector)?)
    }

    pub(crate) fn absent(
        &self,
        parent: Option<&OriginalValueFact<'scope, 'read, 'source>>,
    ) -> Result<FieldOrigin, CatalogError> {
        let input = self.survey(parent)?;
        Ok(self.sources().origin(&[input], true))
    }
}

impl OriginalRecordScope<'_, '_> {
    pub(crate) fn root_declaration_location(&self) -> SourceLocation {
        SourceLocation::Subresource {
            namespace: "dataforgeRecordDeclarations".to_owned(),
            indices: vec![u64::from(self.record().index())],
            field: "$fields".to_owned(),
        }
    }
}
