// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogError;
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession, ProvenanceMode};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

pub(super) const MODE: ProvenanceMode = ProvenanceMode::Full;
pub(super) const NO_LANGUAGES: CatalogLanguageSelection<'static> =
    CatalogLanguageSelection::Selected(&[]);

pub(super) fn open<'archive>(
    archive: &'archive Archive,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<CatalogQuerySession<'archive>, CatalogError> {
    Ok(CatalogQuerySession::open(archive, languages, cancel)?.with_provenance_mode(MODE))
}

pub(super) fn from_entry<'archive>(
    archive: &'archive Archive,
    entry: &'archive DataForgeEntry,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<CatalogQuerySession<'archive>, CatalogError> {
    Ok(
        CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
            .with_provenance_mode(MODE),
    )
}
