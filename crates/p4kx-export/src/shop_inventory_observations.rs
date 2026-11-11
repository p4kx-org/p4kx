// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::CatalogError;
use crate::catalog_read::original::AuthoredEntryPath;

/// Original component evidence beyond a single resolved inventory stem.
#[derive(Debug, Default)]
pub(crate) struct InventoryObservations {
    pub(crate) conflict_paths: Vec<AuthoredEntryPath>,
    pub(crate) rejected: Vec<RejectedInventoryPath>,
    pub(crate) no_file: Vec<AuthoredEntryPath>,
    pub(crate) unscanned: Vec<AuthoredEntryPath>,
}

#[derive(Debug, Clone)]
pub(crate) enum RejectedInventoryPath {
    Text(AuthoredEntryPath),
    Unreadable(AuthoredEntryPath),
}

impl InventoryObservations {
    pub(crate) fn merge(&mut self, mut other: Self) -> Result<(), CatalogError> {
        reserve(&mut self.conflict_paths, other.conflict_paths.len())?;
        reserve(&mut self.rejected, other.rejected.len())?;
        reserve(&mut self.no_file, other.no_file.len())?;
        reserve(&mut self.unscanned, other.unscanned.len())?;
        self.conflict_paths.append(&mut other.conflict_paths);
        self.rejected.append(&mut other.rejected);
        self.no_file.append(&mut other.no_file);
        self.unscanned.append(&mut other.unscanned);
        Ok(())
    }

    pub(crate) fn conflict(&mut self, paths: &[AuthoredEntryPath]) -> Result<(), CatalogError> {
        reserve(&mut self.conflict_paths, paths.len())?;
        self.conflict_paths.extend_from_slice(paths);
        Ok(())
    }

    pub(crate) fn missing_component(
        &mut self,
        metadata: &[AuthoredEntryPath],
    ) -> Result<(), CatalogError> {
        reserve(&mut self.unscanned, metadata.len())?;
        self.unscanned.extend_from_slice(metadata);
        Ok(())
    }
}

fn reserve<T>(paths: &mut Vec<T>, additional: usize) -> Result<(), CatalogError> {
    paths
        .try_reserve(additional)
        .map_err(CatalogError::SourceUseAllocation)
}
