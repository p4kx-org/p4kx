// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to the Catalog's localization Entry metadata.

use p4kx_extraction::{
    Archive, ArchiveWatch, Build, CancelToken, LocalizationNote, SourceInventory, SourceUses,
};

use crate::CatalogError;
use crate::catalog_origins::CatalogLocaleOrigins;
use crate::catalog_query_session::{CatalogLanguageSelection, checkpoint};
use crate::catalog_text_contract::language_sources;
pub use crate::catalog_text_contract::{
    CatalogLanguageSource, CatalogLanguageSources, SourceIniEncoding,
};
use crate::texts::{TextSources, Texts};

/// Observed localization Entries, including expected languages absent from the Archive.
#[non_exhaustive]
pub struct LanguageSourcesSnapshot<'source> {
    /// Exact Build identified by the Archive, when available.
    pub build: Option<&'source Build>,
    /// Canonical source rows for the requested language codes.
    pub sources: CatalogLanguageSources<'source>,
    /// Notes from opening and indexing the requested localization Entries.
    pub notes: Vec<LocalizationNote>,
    /// Physical source references used by the result.
    pub source_inventory: SourceInventory,
}

/// Supplies the Catalog's original localization Entry metadata without opening DataForge.
///
/// # Errors
/// Returns an input, Archive-change, source-evidence or cancellation error.
pub fn with_language_sources<T>(
    archive: &Archive,
    selection: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(LanguageSourcesSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let languages = selection.canonical_languages();
    let inputs = TextSources::read_languages(archive, &languages);
    watch.check()?;
    let inputs = inputs?;
    checkpoint(cancel)?;
    let texts = Texts::parse(&inputs)?;
    let registry = SourceUses::with_compact_coordinates();
    let origins = CatalogLocaleOrigins::read(archive, &inputs, &registry)?;
    let sources = language_sources(&texts, &registry, &origins, &languages)?;
    let source_inventory = registry.finish_with(|| checkpoint(cancel))?;
    watch.check()?;
    checkpoint(cancel)?;
    let result = inspect(LanguageSourcesSnapshot {
        build: archive.build(),
        sources,
        notes: texts.into_notes(),
        source_inventory,
    });
    watch.check()?;
    checkpoint(cancel)?;
    Ok(result)
}
