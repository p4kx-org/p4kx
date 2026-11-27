// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{SourceUseRole, SourceValue};

use super::super::health_value_chain;
use super::{ExampleBodyCapture, ExampleBodyReceipt, ReferenceDatum, ReferenceUse};
use crate::catalog_read::original::{OriginalMissingField, OriginalValuePart};
use crate::record_header_field::RecordHeaderField;
use crate::{CatalogError, ExportAccountingError};

impl<'scope, 'read, 'source> ExampleBodyCapture<'scope, 'read, 'source> {
    pub(crate) fn missing_field(
        &self,
        missing: &OriginalMissingField<'_, 'scope, 'read, 'source>,
    ) -> Result<ExampleBodyReceipt, CatalogError> {
        if !std::ptr::eq(self.original_scope, missing.scope()) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        self.frame.check_scope(missing.scope())?;
        let ancestors = missing.ancestors()?;
        let mut inputs = Vec::new();
        for ancestor in &ancestors {
            super::add_path(&mut inputs, ancestor);
        }
        if let Some(parent) = missing.parent() {
            self.check_fact(parent)?;
            health_value_chain::check(parent, &ancestors)?;
            super::add_path(&mut inputs, parent);
        }
        inputs.push(ReferenceUse::captured(
            missing.survey_location()?,
            SourceUseRole::Selector,
        ));
        let owner = missing.parent().map_or_else(
            || RecordHeaderField::SourceType.location(missing.scope().record().index()),
            |parent| parent.location(OriginalValuePart::ConcreteType),
        );
        Ok(ExampleBodyReceipt::captured(
            Arc::clone(&self.frame),
            ReferenceDatum::captured(
                SourceValue::Present(self.texts.retain_text(missing.path())?.text().clone()),
                inputs,
            ),
            ReferenceDatum::captured(
                SourceValue::Present(
                    self.texts
                        .retain_text(missing.source().type_name())?
                        .text()
                        .clone(),
                ),
                vec![ReferenceUse::captured(owner, SourceUseRole::Selector)],
            ),
            ReferenceDatum::absent(),
        ))
    }
}

#[cfg(test)]
#[path = "health_example_missing_field_tests.rs"]
mod tests;
