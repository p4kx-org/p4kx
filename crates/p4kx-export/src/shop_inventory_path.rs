// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUse;

use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture};
use crate::entry_source::EntrySource;
use crate::{CatalogError, ExportAccountingError};

pub(super) const FOLDER: &str = "data\\scripts\\shopinventories\\";

/// A normalized inventory name with the original selections that produced its requested path.
#[derive(Debug, Clone)]
pub(crate) struct InventoryStem {
    value: String,
    source: Option<Box<EntrySource>>,
    declarations: Vec<InventoryDeclaration>,
}

/// One original inventory declaration and the exact supported stem it selects.
#[derive(Debug, Clone)]
pub(crate) struct InventoryDeclaration {
    pub(crate) original: AuthoredEntryPath,
    pub(crate) stem: AuthoredEntryPath,
    pub(crate) directory_selector: Option<SourceUse>,
}

impl InventoryStem {
    pub(crate) fn captured(
        capture: &EntryCapture<'_>,
        token: &AuthoredEntryPath,
        value: String,
    ) -> Result<Self, CatalogError> {
        Self::with_selector(capture, token, value, None)
    }

    pub(super) fn corrected(
        capture: &EntryCapture<'_>,
        original: &AuthoredEntryPath,
        value: String,
        target: SourceUse,
    ) -> Result<Self, CatalogError> {
        Self::with_selector(capture, original, value, Some(target))
    }

    fn with_selector(
        capture: &EntryCapture<'_>,
        original: &AuthoredEntryPath,
        value: String,
        directory_selector: Option<SourceUse>,
    ) -> Result<Self, CatalogError> {
        let path =
            AuthoredEntryPath::derived(capture, &[original], &format!("{FOLDER}{value}.json"))?;
        let stem = AuthoredEntryPath::derived(capture, &[original], &value)?;
        Ok(Self {
            value,
            source: Some(Box::new(EntrySource::authored_missing(path))),
            declarations: vec![InventoryDeclaration {
                original: original.clone(),
                stem,
                directory_selector,
            }],
        })
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.value
    }

    pub(crate) fn source(&self) -> Result<EntrySource, CatalogError> {
        self.source
            .as_deref()
            .cloned()
            .ok_or_else(|| ExportAccountingError::UnboundHealthSubject.into())
    }

    pub(crate) fn declarations(&self) -> &[InventoryDeclaration] {
        &self.declarations
    }

    pub(crate) fn merge(&mut self, other: &Self) -> Result<(), CatalogError> {
        if self.value != other.value {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        self.declarations
            .try_reserve(other.declarations.len())
            .map_err(CatalogError::SourceUseAllocation)?;
        match (&mut self.source, &other.source) {
            (Some(left), Some(right)) => left.merge(right)?,
            _ => return Err(ExportAccountingError::UnboundHealthSubject.into()),
        }
        self.declarations.extend(other.declarations.iter().cloned());
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn accounting(value: &str) -> Self {
        Self {
            value: value.to_owned(),
            source: None,
            declarations: Vec::new(),
        }
    }
}

impl PartialEq for InventoryStem {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for InventoryStem {}
