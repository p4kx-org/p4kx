// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::OriginalFlowFields;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{ExampleBodyCapture, OriginalValueFact};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

impl<'scope, 'read, 'source> OriginalFlowFields<'scope, 'read, 'source> {
    /// Uses only this inventory's original Scope as the receiver for deferred guard recipes.
    pub(crate) fn body_capture(
        &self,
        input: &'scope OriginalCatalogInput<'_, '_>,
        admission: &RecordObservation<'_, '_>,
    ) -> Result<ExampleBodyCapture<'scope, 'read, 'source>, CatalogError> {
        ExampleBodyCapture::new(input, admission, self.scope)
    }

    pub(crate) fn guard_parent(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<&p4kx_extraction::StructValue<'source>, CatalogError> {
        let ancestors = self.ancestors(fact)?;
        for parent in ancestors.into_iter().rev() {
            if let p4kx_extraction::Value::Struct(source) = parent.value() {
                return Ok(source);
            }
        }
        Ok(self.scope.tree().root())
    }
    /// Returns the actual root-to-parent chain without another source walk or stored path copies.
    pub(crate) fn ancestors(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<Vec<&OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
        self.span(fact)?;
        let mut ancestors = Vec::new();
        let mut child = fact;
        while let Some(parent) = child.parent_ordinal() {
            self.check_cancel()?;
            if parent >= child.ordinal() {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            let parent = self
                .facts
                .get(usize::try_from(parent).map_err(|_| ExportAccountingError::CounterOverflow)?)
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            let actual_child = self.child(Some(parent.ordinal()), child.edge())?;
            if !std::ptr::eq(actual_child.value(), child.value()) {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            ancestors.push(parent);
            child = parent;
        }
        ancestors.reverse();
        Ok(ancestors)
    }
}
