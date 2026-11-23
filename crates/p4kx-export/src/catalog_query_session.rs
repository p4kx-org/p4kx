// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, ArchiveWatch, Build, CancelToken, DataForge, DataForgeEntry, DataForgeLoadError,
    DataForgeOrigin, Language,
};

use crate::CatalogError;
use crate::texts::CATALOG_LANGUAGES;

/// Selects the localization payloads consulted by a catalog query.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum CatalogLanguageSelection<'query> {
    /// Consults every supported language with a source present in the Archive.
    AllAvailable,
    /// Consults these languages only; an empty slice retains keys without translations.
    Selected(&'query [Language]),
}

impl CatalogLanguageSelection<'_> {
    pub(crate) fn canonical_languages(self) -> Vec<Language> {
        match self {
            Self::AllAvailable => CATALOG_LANGUAGES.to_vec(),
            Self::Selected(languages) => CATALOG_LANGUAGES
                .into_iter()
                .filter(|language| languages.contains(language))
                .collect(),
        }
    }
}

/// A selective Catalog reader bound to one opened Archive and its decoded DataForge Entry.
///
/// Each query produces an owned result and its own source inventory. Localization payloads are
/// opened only by queries that need them.
#[must_use]
pub struct CatalogQuerySession<'archive> {
    archive: &'archive Archive,
    watch: ArchiveWatch,
    entry: QueryEntry<'archive>,
    languages: Vec<Language>,
}

/// An opened DataForge view for several selective queries on the same session.
///
/// The view borrows the session's decoded bytes and retains the DataForge's lazy indexes.
#[must_use]
pub struct ParsedCatalogQueries<'session> {
    session: &'session CatalogQuerySession<'session>,
    forge: DataForge<'session>,
}

enum QueryEntry<'archive> {
    Owned(DataForgeEntry),
    Borrowed(&'archive DataForgeEntry),
}

impl QueryEntry<'_> {
    const fn get(&self) -> &DataForgeEntry {
        match self {
            Self::Owned(entry) => entry,
            Self::Borrowed(entry) => entry,
        }
    }
}

impl<'archive> CatalogQuerySession<'archive> {
    /// Opens a session by reading the Archive's DataForge Entry once.
    ///
    /// # Errors
    ///
    /// Returns an input, Archive change, source origin mismatch or cancellation error.
    pub fn open(
        archive: &'archive Archive,
        languages: CatalogLanguageSelection<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        Self::new(archive, None, languages, cancel)
    }

    /// Binds an already decoded DataForge Entry to a query session on the same Archive.
    ///
    /// # Errors
    ///
    /// Returns an Archive change, source origin mismatch or cancellation error.
    pub fn from_entry(
        archive: &'archive Archive,
        entry: &'archive DataForgeEntry,
        languages: CatalogLanguageSelection<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        Self::new(archive, Some(entry), languages, cancel)
    }

    fn new(
        archive: &'archive Archive,
        entry: Option<&'archive DataForgeEntry>,
        languages: CatalogLanguageSelection<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        checkpoint(cancel)?;
        let watch = ArchiveWatch::new(archive);
        watch.check()?;
        let entry = if let Some(entry) = entry {
            QueryEntry::Borrowed(entry)
        } else {
            let decoded = DataForgeEntry::read_until(archive, &|| cancelled(cancel));
            watch.check()?;
            QueryEntry::Owned(decoded.map_err(load_error)?)
        };
        checkpoint(cancel)?;
        if !entry.get().origin().matches_archive(archive) {
            return Err(CatalogError::SourceOriginMismatch);
        }
        Ok(Self {
            archive,
            watch,
            entry,
            languages: languages.canonical_languages(),
        })
    }

    /// Returns the Build identified when the Archive was opened, when available.
    #[must_use]
    pub const fn build(&self) -> Option<&Build> {
        self.archive.build()
    }

    /// Returns the exact physical evidence of the reusable decoded DataForge Entry.
    #[must_use]
    pub const fn dataforge_origin(&self) -> &DataForgeOrigin {
        self.entry.get().origin()
    }

    /// Returns the session's requested languages in canonical order, including absent languages.
    #[must_use]
    pub fn languages(&self) -> &[Language] {
        &self.languages
    }

    /// Opens a reusable parsed view over this session's decoded DataForge bytes.
    ///
    /// # Errors
    ///
    /// Returns a format, Archive change or cancellation error.
    pub fn parsed(
        &self,
        cancel: Option<&CancelToken>,
    ) -> Result<ParsedCatalogQueries<'_>, CatalogError> {
        checkpoint(cancel)?;
        self.watch.check()?;
        let forge = self.entry.get().open().map_err(CatalogError::DataForge);
        self.watch.check()?;
        checkpoint(cancel)?;
        Ok(ParsedCatalogQueries {
            session: self,
            forge: forge?,
        })
    }

    pub(crate) fn project<T>(
        &self,
        cancel: Option<&CancelToken>,
        project: impl FnOnce(&Archive, &DataForgeEntry) -> Result<T, CatalogError>,
    ) -> Result<T, CatalogError> {
        checkpoint(cancel)?;
        self.watch.check()?;
        let result = project(self.archive, self.entry.get());
        self.watch.check()?;
        checkpoint(cancel)?;
        result
    }
}

impl ParsedCatalogQueries<'_> {
    pub(crate) fn languages(&self) -> &[Language] {
        self.session.languages()
    }

    pub(crate) fn project<T>(
        &self,
        cancel: Option<&CancelToken>,
        project: impl FnOnce(&Archive, &DataForgeEntry, &DataForge<'_>) -> Result<T, CatalogError>,
    ) -> Result<T, CatalogError> {
        self.session.project(cancel, |archive, entry| {
            project(archive, entry, &self.forge)
        })
    }
}

pub(crate) fn checkpoint(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancelled(cancel) {
        Err(CatalogError::Cancelled)
    } else {
        Ok(())
    }
}

pub(crate) fn load_error(error: DataForgeLoadError) -> CatalogError {
    match error {
        DataForgeLoadError::Cancelled => CatalogError::Cancelled,
        other => CatalogError::Load(other),
    }
}

fn cancelled(cancel: Option<&CancelToken>) -> bool {
    cancel.is_some_and(CancelToken::is_cancelled)
}

#[cfg(test)]
#[path = "catalog_query_session_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "installed_sdk_witness.rs"]
mod installed_sdk_witness;
