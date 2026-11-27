// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;

use super::OriginalFlowFields;
use crate::catalog_read::original::{OriginalValueEdge, OriginalValueFact};
use crate::{CatalogError, ExportAccountingError};

impl<'scope, 'read, 'source> OriginalFlowFields<'scope, 'read, 'source> {
    /// Builds a diagnostic path only on demand from the retained actual parent edges.
    pub(crate) fn logical_path(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<String, CatalogError> {
        self.span(fact)?;
        let mut ancestors = Vec::new();
        let mut at = fact;
        loop {
            ancestors.push(at);
            let Some(parent) = at.parent_ordinal() else {
                break;
            };
            if parent >= at.ordinal() {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            at = self
                .facts
                .get(usize::try_from(parent).map_err(|_| ExportAccountingError::CounterOverflow)?)
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        }
        let mut path = String::new();
        for ancestor in ancestors.into_iter().rev() {
            match ancestor.edge() {
                OriginalValueEdge::Field { .. } => {
                    if !path.is_empty() {
                        path.push('.');
                    }
                    path.push_str(ancestor.field().name());
                }
                OriginalValueEdge::Element { ordinal } => {
                    write!(path, "[{ordinal}]")
                        .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
                }
            }
        }
        Ok(path)
    }
}
