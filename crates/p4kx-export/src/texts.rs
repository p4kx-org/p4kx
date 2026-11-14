// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_extraction::{
    Archive, Language, LocalizationEntry, LocalizationLoadError, LocalizationNote, LocalizationSet,
    TextSet, display_text, is_placeholder_key,
};

use crate::contract::LocalisedText;
use crate::{CatalogError, TextSummary};

/// The languages the catalog carries texts in, in the order its summary lists them.
pub(crate) const CATALOG_LANGUAGES: [Language; 11] = Language::ALL;

/// The `global.ini` Entry of every language in [`CATALOG_LANGUAGES`] the Archive holds.
pub(crate) struct TextSources {
    set: LocalizationSet,
}

impl TextSources {
    /// Reads the `global.ini` of every catalog language from `archive`.
    pub(crate) fn read(archive: &Archive) -> Result<Self, CatalogError> {
        let set = LocalizationSet::read_all(archive).map_err(CatalogError::LoadTexts)?;
        Ok(Self { set })
    }

    pub(crate) fn entry(&self, language: Language) -> Option<&LocalizationEntry> {
        self.set.entry(language)
    }
}

/// Resolves localisation keys in every catalog language and counts what resolves.
///
/// Every kind turns its keys into [`LocalisedText`]s through [`Texts::localised`]; it may be
/// called from any worker thread.
pub(crate) struct Texts<'e> {
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
            set,
            notes,
            languages: CATALOG_LANGUAGES.map(LanguageCount::new),
            placeholders: AtomicUsize::new(0),
        })
    }

    /// Returns the text with localisation `key` and its English and German texts, each
    /// decoded (`\n` to a line feed) and trimmed; without a key every part is `None` and nothing is counted, and a
    /// [placeholder key](crate::PLACEHOLDER_KEYS) keeps the key, has no texts and is counted as a
    /// placeholder.
    pub(crate) fn localised<'a>(&self, key: Option<&'a str>) -> LocalisedText<'a>
    where
        'e: 'a,
    {
        let Some(key) = key else {
            return LocalisedText::default();
        };
        if is_placeholder_key(key) {
            self.placeholders.fetch_add(1, Ordering::Relaxed);
            return LocalisedText {
                key: Some(key),
                ..LocalisedText::default()
            };
        }
        LocalisedText {
            key: Some(key),
            en: self.resolve(Language::English, key),
            de: self.resolve(Language::German, key),
        }
    }

    /// Returns a note for every `global.ini` that was read lossily or had lines skipped.
    pub(crate) fn notes(&self) -> &[LocalizationNote] {
        &self.notes
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
        let text = if is_placeholder_key(key) {
            None
        } else {
            self.set.text(key, language).map(|source| TextProjection {
                source,
                display: display_text(source),
            })
        };
        if text.as_ref().is_some_and(|text| !text.display.is_empty()) {
            count.resolved.fetch_add(1, Ordering::Relaxed);
        } else {
            count.unresolved.fetch_add(1, Ordering::Relaxed);
        }
        text
    }

    pub(crate) fn note_placeholder(&self) {
        self.placeholders.fetch_add(1, Ordering::Relaxed);
    }

    fn resolve(&self, language: Language, key: &str) -> Option<Cow<'e, str>> {
        self.project(language, key)
            .map(|text| text.display)
            .filter(|text| !text.is_empty())
    }
}
