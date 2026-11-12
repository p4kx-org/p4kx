// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{CancelToken, RecordTree, Value};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::OriginalValueSite;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError, Kind};

use super::inspect_values;

/// Inspects the exact original root value a reader uses from a secondary Record.
pub(crate) fn inspect_record_field<'source>(
    input: &OriginalCatalogInput<'_, '_>,
    tree: &RecordTree<'source>,
    admission: &RecordObservation<'_, 'source>,
    kind: Kind,
    name: &'static str,
    cancel: Option<&CancelToken>,
) -> Result<ExportLedger, CatalogError> {
    inspect_values(
        Some(input),
        tree,
        admission,
        Some(kind),
        ExportSourceRole::Dependency,
        cancel,
        &FieldSelection::RecordField {
            name,
            value: tree.field(name),
        },
    )
}

pub(super) enum FieldSelection<'read, 'source> {
    All,
    Subtree(Option<Range<usize>>),
    RecordField {
        name: &'static str,
        value: Option<&'read Value<'source>>,
    },
}

impl FieldSelection<'_, '_> {
    pub(super) fn includes(
        &self,
        site: &OriginalValueSite<'_, '_, '_, '_>,
    ) -> Result<bool, ExportAccountingError> {
        match self {
            Self::All => Ok(true),
            Self::Subtree(None) | Self::RecordField { value: None, .. } => Ok(false),
            Self::Subtree(Some(span)) => {
                let index = usize::try_from(site.ordinal())
                    .map_err(|_| ExportAccountingError::CounterOverflow)?;
                Ok(span.contains(&index))
            }
            Self::RecordField {
                value: Some(value), ..
            } => Ok(std::ptr::eq(*value, site.value())),
        }
    }

    pub(super) const fn is_empty(&self) -> bool {
        matches!(
            self,
            Self::Subtree(None) | Self::RecordField { value: None, .. }
        )
    }

    pub(super) const fn root_field(&self) -> Option<&'static str> {
        match self {
            Self::RecordField { name, .. } => Some(name),
            Self::All | Self::Subtree(_) => None,
        }
    }
}
