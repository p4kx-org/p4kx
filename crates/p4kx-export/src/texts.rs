// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_extraction::{
    Archive, Language, LocalizationEntry, LocalizationLoadError, LocalizationNote, LocalizationSet,
    TextSet, display_text, is_placeholder_key,
};

use crate::{CatalogError, TextSummary};

/// The languages the catalog carries texts in, in the order its summary lists them.
pub(crate) const CATALOG_LANGUAGES: [Language; 11] = Language::ALL;

/// The `global.ini` Entry of every language in [`CATALOG_LANGUAGES`] the Archive holds.
pub(crate) struct TextSources {
    set: LocalizationSet,
    consulted: Vec<Language>,
}

impl TextSources {
    /// Reads the `global.ini` of every catalog language from `archive`.
    pub(crate) fn read(archive: &Archive) -> Result<Self, CatalogError> {
        Self::read_item_languages(archive, &CATALOG_LANGUAGES)
    }

    pub(crate) fn read_item_languages(
        archive: &Archive,
        requested: &[Language],
    ) -> Result<Self, CatalogError> {
        let languages: Vec<_> = CATALOG_LANGUAGES
            .into_iter()
            .filter(|language| *language == Language::English || requested.contains(language))
            .collect();
        Self::read_languages(archive, &languages)
    }

    pub(crate) fn read_languages(
        archive: &Archive,
        languages: &[Language],
    ) -> Result<Self, CatalogError> {
        let set = LocalizationSet::read(archive, languages).map_err(CatalogError::LoadTexts)?;
        Ok(Self {
            set,
            consulted: languages.to_vec(),
        })
    }

    pub(crate) fn consulted(&self, language: Language) -> bool {
        self.consulted.contains(&language)
    }

    pub(crate) fn entry(&self, language: Language) -> Option<&LocalizationEntry> {
        self.set.entry(language)
    }
}

/// Resolves localisation keys in every catalog language and counts what resolves.
pub(crate) struct Texts<'e> {
    sources: &'e TextSources,
    set: TextSet<'e>,
    notes: Vec<LocalizationNote>,
    languages: [LanguageCount; CATALOG_LANGUAGES.len()],
    placeholders: AtomicUsize,
}

/// One winning decoded text and its display form from the same lookup.
pub(crate) struct TextProjection<'source> {
    pub(crate) source: &'source str,
    pub(crate) display: Cow<'source, str>,
}

struct LanguageCount {
    language: Language,
    resolved: AtomicUsize,
    unresolved: AtomicUsize,
}

impl LanguageCount {
    const fn new(language: Language) -> Self {
        Self {
            language,
            resolved: AtomicUsize::new(0),
            unresolved: AtomicUsize::new(0),
        }
    }

    fn note(&self, resolved: bool) {
        let counter = if resolved {
            &self.resolved
        } else {
            &self.unresolved
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

impl<'e> Texts<'e> {
    /// Indexes every language of `sources` in parallel.
    pub(crate) fn parse(sources: &'e TextSources) -> Result<Self, CatalogError> {
        let set = sources.set.index().map_err(|error| match error {
            LocalizationLoadError::Index { path, source } => CatalogError::Texts { path, source },
            other => CatalogError::LoadTexts(other),
        })?;
        let notes = sources.set.notes(&set);
        Ok(Self {
            sources,
            set,
            notes,
            languages: CATALOG_LANGUAGES.map(LanguageCount::new),
            placeholders: AtomicUsize::new(0),
        })
    }

    pub(crate) fn consulted(&self, language: Language) -> bool {
        self.sources.consulted(language)
    }

    pub(crate) fn source_entry(&self, language: Language) -> Option<&'e LocalizationEntry> {
        self.sources.entry(language)
    }

    pub(crate) fn skipped_lines(&self, language: Language) -> Option<usize> {
        self.set
            .ini(language)
            .map(p4kx_extraction::GlobalIni::skipped)
    }

    pub(crate) fn notes(&self) -> &[LocalizationNote] {
        &self.notes
    }

    /// Drops the lookup indexes and returns the language counts and localization notes.
    pub(crate) fn into_summary(self) -> (Vec<TextSummary>, Vec<LocalizationNote>) {
        let summary = self.summary();
        (summary, self.into_notes())
    }

    /// Returns localization notes without preparing the field-count summary.
    pub(crate) fn into_notes(self) -> Vec<LocalizationNote> {
        self.notes
    }

    /// Returns what resolved per language, in the order of [`CATALOG_LANGUAGES`].
    pub(crate) fn summary(&self) -> Vec<TextSummary> {
        let placeholder = self.placeholders.load(Ordering::Relaxed);
        self.languages
            .iter()
            .filter(|count| {
                matches!(count.language, Language::English | Language::German)
                    || self.set.ini(count.language).is_some()
            })
            .map(|count| TextSummary {
                language: count.language,
                available: self.set.ini(count.language).is_some(),
                resolved: count.resolved.load(Ordering::Relaxed),
                unresolved: count.unresolved.load(Ordering::Relaxed),
                placeholder,
            })
            .collect()
    }

    /// Projects both forms of one winning text and counts its display result once.
    pub(crate) fn project(&self, language: Language, key: &str) -> Option<TextProjection<'e>> {
        let count = self
            .languages
            .iter()
            .find(|count| count.language == language)?;
        let text = self.lookup_forms(language, key);
        count.note(text.as_ref().is_some_and(|text| !text.display.is_empty()));
        text
    }

    /// Counts one lookup occurrence per supported language from prepared outcomes.
    pub(crate) fn note_languages(&self, resolved: [bool; CATALOG_LANGUAGES.len()]) {
        for (count, resolved) in self.languages.iter().zip(resolved) {
            count.note(resolved);
        }
    }

    /// Returns the winning forms without recording another lookup.
    pub(crate) fn lookup_forms(&self, language: Language, key: &str) -> Option<TextProjection<'e>> {
        if is_placeholder_key(key) {
            return None;
        }
        let source = self.set.text(key, language)?;
        Some(TextProjection {
            source,
            display: display_text(source),
        })
    }

    pub(crate) fn note_placeholder(&self) {
        self.placeholders.fetch_add(1, Ordering::Relaxed);
    }
}
