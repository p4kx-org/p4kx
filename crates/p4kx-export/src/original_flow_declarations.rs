// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;

use super::OriginalFlowFields;
use crate::catalog_read::original::{OriginalValueEdge, OriginalValueFact};
use crate::{CatalogError, ExportAccountingError};

impl<'scope, 'read, 'source> OriginalFlowFields<'scope, 'read, 'source> {
    pub(crate) fn declarations(
        &self,
        parent: Option<&OriginalValueFact<'scope, 'read, 'source>>,
        name: &str,
    ) -> Result<Vec<&OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
        let source = match parent {
            Some(fact) => {
                self.span(fact)?;
                let Value::Struct(source) = fact.value() else {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                };
                source
            }
            None => self.root(),
        };
        source
            .fields()
            .iter()
            .enumerate()
            .filter(|(_, field)| field.name() == name)
            .map(|(index, field)| {
                let fact = self.child(
                    parent.map(OriginalValueFact::ordinal),
                    OriginalValueEdge::Field {
                        ordinal: u64::try_from(index)
                            .map_err(|_| ExportAccountingError::CounterOverflow)?,
                    },
                )?;
                if !std::ptr::eq(field.value(), fact.value()) {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
                Ok(fact)
            })
            .collect()
    }
}
