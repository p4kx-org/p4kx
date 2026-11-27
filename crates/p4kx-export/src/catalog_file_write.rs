// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::{Archive, DataForgeEntry};

use crate::build;
use crate::options::{CatalogRun, CatalogStage};
use crate::{CatalogError, CatalogFile, CatalogOutputOptions, CatalogSummary, Compression};

/// Streams a catalog into a lazy created file and explicitly cleans failed unpublished output.
///
/// # Errors
/// Returns build/write failures or a separate cleanup failure retaining its original cause.
pub fn write_catalog_file<'a, 'run, 'inputs: 'run>(
    archive: &Archive,
    options: impl Into<CatalogOutputOptions<'run, 'inputs>>,
    compression: Compression,
    create: impl FnOnce() -> io::Result<CatalogFile<'a>>,
) -> Result<(CatalogSummary, CatalogFile<'a>), CatalogError> {
    write_archive(
        archive,
        CatalogRun::for_output(options, None),
        compression,
        create,
    )
}

/// Writes a catalog file and reports each stage before it runs on the calling thread.
///
/// # Errors
/// Returns the same build, write or cleanup errors as [`write_catalog_file`].
pub fn write_catalog_file_with_phases<'a, 'run, 'inputs: 'run>(
    archive: &Archive,
    options: impl Into<CatalogOutputOptions<'run, 'inputs>>,
    compression: Compression,
    report: &'run dyn Fn(CatalogStage),
    create: impl FnOnce() -> io::Result<CatalogFile<'a>>,
) -> Result<(CatalogSummary, CatalogFile<'a>), CatalogError> {
    write_archive(
        archive,
        CatalogRun::for_output(options, Some(report)),
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
    let entry = build::api::read_archive_entry(archive, run)?;
    write_file_from(archive, &entry, run, compression, create)
}

/// Streams from an already loaded DataForge while retaining the concrete output until completion.
///
/// # Errors
/// Returns build/write failures or a cleanup failure without hiding cancellation or health errors.
pub fn write_catalog_file_from<'a, 'run, 'inputs: 'run>(
    archive: &Archive,
    entry: &DataForgeEntry,
    options: impl Into<CatalogOutputOptions<'run, 'inputs>>,
    compression: Compression,
    create: impl FnOnce() -> io::Result<CatalogFile<'a>>,
) -> Result<(CatalogSummary, CatalogFile<'a>), CatalogError> {
    write_file_from(
        archive,
        entry,
        CatalogRun::for_output(options, None),
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
            Some(file) => Err(file.discard_failure(primary)),
            None => Err(primary),
        },
    }
}
