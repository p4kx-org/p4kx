// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::{Archive, DataForgeEntry};

use crate::{CatalogError, CatalogFile, CatalogOptions, CatalogSummary, Compression};

/// Streams a catalog into a lazy created file and explicitly cleans failed unpublished output.
///
/// # Errors
/// Returns build/write failures or a separate cleanup failure retaining its original cause.
pub fn write_catalog_file<'a>(
    archive: &Archive,
    options: &CatalogOptions<'_>,
    compression: Compression,
    create: impl FnOnce() -> io::Result<CatalogFile<'a>>,
) -> Result<(CatalogSummary, CatalogFile<'a>), CatalogError> {
    if options
        .cancel
        .is_some_and(p4kx_extraction::CancelToken::is_cancelled)
    {
        return Err(CatalogError::Cancelled);
    }
    let entry = DataForgeEntry::read(archive).map_err(CatalogError::Load)?;
    write_catalog_file_from(archive, &entry, options, compression, create)
}

/// Streams from an already loaded DataForge while retaining the concrete output until completion.
///
/// # Errors
/// Returns build/write failures or a cleanup failure without hiding cancellation or health errors.
pub fn write_catalog_file_from<'a>(
    archive: &Archive,
    entry: &DataForgeEntry,
    options: &CatalogOptions<'_>,
    compression: Compression,
    create: impl FnOnce() -> io::Result<CatalogFile<'a>>,
) -> Result<(CatalogSummary, CatalogFile<'a>), CatalogError> {
    let mut file = None;
    let result = crate::write_catalog_from(archive, entry, options, compression, || {
        Ok(file.insert(create()?))
    });
    match result {
        Ok((summary, _)) => file
            .map(|file| (summary, file))
            .ok_or_else(|| CatalogError::Write(io::Error::other("catalog output was not created"))),
        Err(primary) => match file {
            Some(file) => match file.discard() {
                Ok(()) => Err(primary),
                Err(cleanup) => Err(CatalogError::CleanupFailed {
                    primary: Box::new(primary),
                    cleanup: Box::new(cleanup),
                }),
            },
            None => Err(primary),
        },
    }
}
