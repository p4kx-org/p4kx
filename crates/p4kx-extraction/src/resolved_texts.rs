// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{ObservedResolution, Resolution, TextResolver};
use p4kx_localization::{Language, OwnedTextSet, TextSet};
use p4kx_localization::{decode_line_breaks, is_placeholder_key};

#[path = "indexed_record_localization.rs"]
mod indexed_record_localization;
#[path = "record_localization.rs"]
mod record_localization;

pub use indexed_record_localization::IndexedRecordLocalization;
pub(crate) use record_localization::LocalizationLookupEvidence;
pub use record_localization::RecordLocalization;

#[derive(Debug, Clone, Copy)]
enum Source<'t, 'a> {
    Borrowed(&'t TextSet<'a>),
    Owned(&'t OwnedTextSet),
}

/// The texts of one language of a [`TextSet`] or an [`OwnedTextSet`], to write into Record XML
/// and Record JSON.
///
/// A key without a text in the language is missing; a placeholder key stays alone. The `\n`
/// escapes of the `global.ini` become line feeds.
#[derive(Debug, Clone, Copy)]
pub struct LanguageTexts<'t, 'a> {
    source: Source<'t, 'a>,
    language: Language,
}

impl<'t, 'a> LanguageTexts<'t, 'a> {
    /// Resolves in `language` with the files of `texts`.
    #[must_use]
    pub const fn new(texts: &'t TextSet<'a>, language: Language) -> Self {
        Self {
            source: Source::Borrowed(texts),
            language,
        }
    }

    /// Resolves in `language` with the texts of `texts`, which borrow nothing.
    #[must_use]
    pub const fn owned(texts: &'t OwnedTextSet, language: Language) -> Self {
        Self {
            source: Source::Owned(texts),
            language,
        }
    }

    /// Returns the language keys are resolved in.
    #[must_use]
    pub const fn language(&self) -> Language {
        self.language
    }
}

impl TextResolver for LanguageTexts<'_, '_> {
    fn resolve<'r>(&'r self, key: &str) -> Resolution<'r> {
        self.resolve_observed(key).into_resolution()
    }

    fn resolve_observed<'r>(&'r self, key: &str) -> ObservedResolution<'r> {
        if is_placeholder_key(key) {
            return ObservedResolution::unobserved(Resolution::Keep);
        }
        match self.source {
            Source::Borrowed(texts) => {
                let Some(ini) = texts.ini(self.language) else {
                    return ObservedResolution::unobserved(Resolution::Missing);
                };
                let original = ini.resolve(key);
                ObservedResolution::from_lookup(display(original), original)
            }
            Source::Owned(texts) => {
                let original = texts.text(key, self.language);
                ObservedResolution::unobserved(display(original))
            }
        }
    }
}

fn display(original: Option<&str>) -> Resolution<'_> {
    match original {
        Some(text) if !text.is_empty() => Resolution::Text(decode_line_breaks(text)),
        _ => Resolution::Missing,
    }
}
