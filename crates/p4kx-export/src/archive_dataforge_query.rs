// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, ArchiveWatch, CancelToken, DataForgeEntry, DataForgeLoadError};

use crate::CatalogError;

pub(crate) fn read<T>(
    archive: &Archive,
    entry: Option<&DataForgeEntry>,
    cancel: Option<&CancelToken>,
    project: impl FnOnce(&DataForgeEntry) -> Result<T, CatalogError>,
) -> Result<T, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    check_archive(&watch)?;
    let decoded;
    let entry = if let Some(entry) = entry {
        entry
    } else {
        decoded = DataForgeEntry::read_until(archive, &|| cancelled(cancel)).map_err(load_error)?;
        &decoded
    };
    checkpoint(cancel)?;
    check_archive(&watch)?;
    if !entry.origin().matches_archive(archive) {
        return Err(CatalogError::SourceOriginMismatch);
    }
    let result = project(entry)?;
    check_archive(&watch)?;
    checkpoint(cancel)?;
    Ok(result)
}

pub(crate) fn checkpoint(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancelled(cancel) {
        Err(CatalogError::Cancelled)
    } else {
        Ok(())
    }
}

fn check_archive(watch: &ArchiveWatch) -> Result<(), CatalogError> {
    watch
        .check()
        .map_err(|error| CatalogError::Load(DataForgeLoadError::ArchiveChanged(error)))
}

fn load_error(error: DataForgeLoadError) -> CatalogError {
    match error {
        DataForgeLoadError::Cancelled => CatalogError::Cancelled,
        other => CatalogError::Load(other),
    }
}

fn cancelled(cancel: Option<&CancelToken>) -> bool {
    cancel.is_some_and(CancelToken::is_cancelled)
}
