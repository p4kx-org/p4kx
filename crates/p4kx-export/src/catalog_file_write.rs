// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::{Archive, DataForgeEntry};

use crate::build;
use crate::options::{CatalogRun, CatalogStage};
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
    write_archive(archive, CatalogRun::new(options, None), compression, create)
}

/// Writes a catalog file and reports each stage before it runs on the calling thread.
///
/// # Errors
/// Returns the same build, write or cleanup errors as [`write_catalog_file`].
pub fn write_catalog_file_with_phases<'a>(
    archive: &Archive,
    options: &CatalogOptions<'_>,
    compression: Compression,
    report: &dyn Fn(CatalogStage),
    create: impl FnOnce() -> io::Result<CatalogFile<'a>>,
) -> Result<(CatalogSummary, CatalogFile<'a>), CatalogError> {
    write_archive(
        archive,
        CatalogRun::new(options, Some(report)),
        compression,
        create,
    )
}

fn write_archive<'a>(
    archive: &Archive,
    run: CatalogRun<'_, '_>,
    compression: Compression,
    create: impl FnOnce() -> io::Result<CatalogFile<'a>>,
) -> Result<(CatalogSummary, CatalogFile<'a>), CatalogError> {
    if run
        .options
        .cancel
        .is_some_and(p4kx_extraction::CancelToken::is_cancelled)
    {
        return Err(CatalogError::Cancelled);
    }
    run.begin(CatalogStage::ReadDataForge);
    let entry = DataForgeEntry::read(archive).map_err(CatalogError::Load)?;
    write_file_from(archive, &entry, run, compression, create)
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
    write_file_from(
        archive,
        entry,
        CatalogRun::new(options, None),
        compression,
        create,
    )
}

fn write_file_from<'a>(
    archive: &Archive,
    entry: &DataForgeEntry,
    run: CatalogRun<'_, '_>,
    compression: Compression,
    create: impl FnOnce() -> io::Result<CatalogFile<'a>>,
) -> Result<(CatalogSummary, CatalogFile<'a>), CatalogError> {
    let mut file = None;
    let result = build::api::write_catalog_from_with_run(archive, entry, run, compression, || {
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
