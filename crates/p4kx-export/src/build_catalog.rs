// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::ops::ControlFlow;

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use crate::catalog_output::{Compression, emit};
use crate::contract::Catalog;
use crate::options::CatalogRun;
use crate::{BuiltCatalog, CatalogError, CatalogOptions, CatalogSummary};

use super::parts::Parts;

#[cfg(test)]
#[path = "build_catalog_size_witness.rs"]
mod size_witness;

#[cfg(test)]
#[path = "../tests/real_archive/real_archive_resource_oracles.rs"]
mod resource_oracle;
#[cfg(test)]
#[path = "build_resource_oracle.rs"]
mod resource_projection;

/// Builds a deterministic P4KX catalog from the DataForge and available localization inputs.
///
/// Resource definitions retain per-value source origins and canonical text references.
/// Normal publication requires complete export health; diagnostic mode retains available rows.
///
/// # Errors
///
/// Returns an input, projection, accounting or cancellation error when the catalog cannot be built.
/// Returns [`CatalogError::IncompleteExport`] when normal publication is unsafe.
pub fn build_catalog(
    archive: &Archive,
    options: &CatalogOptions<'_>,
) -> Result<BuiltCatalog, CatalogError> {
    if options.cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let entry = DataForgeEntry::read(archive).map_err(CatalogError::Load)?;
    build_catalog_from(archive, &entry, options)
}

/// Builds the P4KX catalog like [`build_catalog`] and streams it like [`write_catalog_from`].
///
/// # Errors
///
/// As [`build_catalog`] and [`write_catalog_from`].
pub fn write_catalog<W: Write>(
    archive: &Archive,
    options: &CatalogOptions<'_>,
    compression: Compression,
    create: impl FnOnce() -> io::Result<W>,
) -> Result<(CatalogSummary, W), CatalogError> {
    if options.cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let entry = DataForgeEntry::read(archive).map_err(CatalogError::Load)?;
    write_catalog_from(archive, &entry, options, compression, create)
}

/// Builds the P4KX catalog like [`build_catalog`] from `entry`, the DataForge Entry already
/// read from `archive`, so that a caller holding it does not read it again.
///
/// # Errors
///
/// As [`build_catalog`], except that the DataForge Entry is not read here.
pub fn build_catalog_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    options: &CatalogOptions<'_>,
) -> Result<BuiltCatalog, CatalogError> {
    let run = CatalogRun::new(options, None);
    let (parts, written) = assemble(archive, entry, run, |catalog| {
        emit(catalog, Vec::new(), Compression::None, run)
    })?;
    Ok(BuiltCatalog {
        summary: parts.summary(&written),
        json: written.sink,
    })
}

/// Builds the P4KX catalog like [`build_catalog_from`] and streams its JSON into the writer
/// `create` returns, compressed as `compression` says.
///
/// The JSON is never held in memory; zstd serialises it twice, once to count its length for the
/// frame header.
///
/// `create` is called once after the build passes its publication policy, before the first byte
/// is written; a refused or cancelled build creates nothing. Returns the summary, whose
/// [`size`](CatalogSummary::size) is the JSON size and whose
/// [`compressed_size`](CatalogSummary::compressed_size) is the size written when compressed, and
/// the writer, flushed and with the compressed frame finished.
///
/// # Errors
///
/// As [`build_catalog`], except that the DataForge Entry is not read here, plus
/// [`CatalogError::Write`] when `create` or the writer fails and [`CatalogError::Cancelled`] when
/// the [`CancelToken`] of `options` is cancelled while writing.
pub fn write_catalog_from<W: Write>(
    archive: &Archive,
    entry: &DataForgeEntry,
    options: &CatalogOptions<'_>,
    compression: Compression,
    create: impl FnOnce() -> io::Result<W>,
) -> Result<(CatalogSummary, W), CatalogError> {
    write_catalog_from_with_run(
        archive,
        entry,
        CatalogRun::new(options, None),
        compression,
        create,
    )
}

pub(crate) fn write_catalog_from_with_run<W: Write>(
    archive: &Archive,
    entry: &DataForgeEntry,
    run: CatalogRun<'_, '_>,
    compression: Compression,
    create: impl FnOnce() -> io::Result<W>,
) -> Result<(CatalogSummary, W), CatalogError> {
    let (parts, written) = assemble(archive, entry, run, |catalog| {
        let out = create().map_err(CatalogError::Write)?;
        emit(catalog, out, compression, run)
    })?;
    Ok((parts.summary(&written), written.sink))
}

fn assemble<R>(
    archive: &Archive,
    entry: &DataForgeEntry,
    run: CatalogRun<'_, '_>,
    emit: impl FnOnce(&Catalog<'_>) -> Result<R, CatalogError>,
) -> Result<(Parts, R), CatalogError> {
    match super::assemble(archive, entry, run, emit)? {
        ControlFlow::Continue(built) => Ok(built),
        ControlFlow::Break(refusal) => Err(CatalogError::IncompleteExport(refusal.freeze(run)?)),
    }
}
