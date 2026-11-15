// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{CancelToken, RecordTree, Value};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::ProtectedKey;
use crate::catalog_read::original::OriginalValueSite;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_field_inspection::ConfigurationInspection;
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
    inspect_one_field(input, tree, admission, kind, name, cancel, None)
}

/// Inspects a protected key and retains the exact same selected source occurrence.
pub(crate) fn inspect_protected_key<'source>(
    input: &OriginalCatalogInput<'_, '_>,
    tree: &RecordTree<'source>,
    admission: &RecordObservation<'_, 'source>,
    kind: Kind,
    name: &'static str,
    cancel: Option<&CancelToken>,
) -> Result<(ExportLedger, ProtectedKey<'source>), CatalogError> {
    let mut key = None;
    let ledger = inspect_one_field(input, tree, admission, kind, name, cancel, Some(&mut key))?;
    let key = match (tree.field(name), key) {
        (Some(_), Some(key)) => key,
        (None, None) => {
            ProtectedKey::absent_from_root(&input.record_scope(admission.record(), tree)?)?
        }
        _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
    };
    Ok((ledger, key))
}

fn inspect_one_field<'source>(
    input: &OriginalCatalogInput<'_, '_>,
    tree: &RecordTree<'source>,
    admission: &RecordObservation<'_, 'source>,
    kind: Kind,
    name: &'static str,
    cancel: Option<&CancelToken>,
    key: Option<&mut Option<ProtectedKey<'source>>>,
) -> Result<ExportLedger, CatalogError> {
    inspect_values(
        Some(input),
        tree,
        admission,
        Some(kind),
        ExportSourceRole::Dependency,
        cancel,
        &mut FieldSelection::RecordField {
            name,
            value: tree.field(name),
            key,
        },
    )
}

pub(super) enum FieldSelection<'read, 'source> {
    All,
    Subtrees {
        spans: &'read [Range<usize>],
        configuration: ConfigurationInspection,
    },
    RecordField {
        name: &'static str,
        value: Option<&'read Value<'source>>,
        key: Option<&'read mut Option<ProtectedKey<'source>>>,
    },
}

impl<'source> FieldSelection<'_, 'source> {
    pub(super) fn select(
        &mut self,
        site: &OriginalValueSite<'_, '_, '_, 'source>,
    ) -> Result<bool, CatalogError> {
        match self {
            Self::All => Ok(true),
            Self::RecordField { value: None, .. } => Ok(false),
            Self::Subtrees { spans, .. } => {
                let index = usize::try_from(site.ordinal())
                    .map_err(|_| ExportAccountingError::CounterOverflow)?;
                let position = spans.partition_point(|span| span.end <= index);
                Ok(spans
                    .get(position)
                    .is_some_and(|span| span.contains(&index)))
            }
            Self::RecordField {
                value: Some(value),
                key,
                ..
            } => {
                let selected = std::ptr::eq(*value, site.value());
                if selected && let Some(key) = key {
                    **key = Some(ProtectedKey::from_fact(&site.fact())?);
                }
                Ok(selected)
            }
        }
    }

    pub(super) const fn is_empty(&self) -> bool {
        match self {
            Self::Subtrees { spans, .. } => spans.is_empty(),
            Self::RecordField { value, .. } => value.is_none(),
            Self::All => false,
        }
    }

    pub(super) const fn configuration(&self) -> Option<ConfigurationInspection> {
        match self {
            Self::Subtrees { configuration, .. } => Some(*configuration),
            Self::RecordField { .. } | Self::All => None,
        }
    }

    pub(super) const fn root_field(&self) -> Option<&'static str> {
        match self {
            Self::RecordField { name, .. } => Some(name),
            Self::All | Self::Subtrees { .. } => None,
        }
    }
}
