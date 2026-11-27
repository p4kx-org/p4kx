// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;
use std::collections::TryReserveError;

use crate::lookup::{Lookup, resolve};
use crate::{GlobalIni, Language, OwnedTextSet};

/// The indexed `global.ini` files of several languages, to resolve keys with a fallback language
/// and to list every key with its texts.
#[derive(Debug, Clone, Default)]
pub struct TextSet<'a> {
    languages: Vec<(Language, GlobalIni<'a>)>,
}

impl<'a> TextSet<'a> {
    /// Creates the set from the indexed files; of two files for one language the later wins.
    #[must_use]
    pub fn new(files: impl IntoIterator<Item = (Language, GlobalIni<'a>)>) -> Self {
        let mut languages: Vec<(Language, GlobalIni<'a>)> = Vec::new();
        for (language, ini) in files {
            match languages.iter_mut().find(|(known, _)| *known == language) {
                Some(slot) => slot.1 = ini,
                None => languages.push((language, ini)),
            }
        }
        languages.sort_by_key(|(language, _)| *language);
        Self { languages }
    }

    /// Returns the languages the set holds a file for, in the order of [`Language::ALL`].
    pub fn languages(&self) -> impl Iterator<Item = Language> {
        self.languages.iter().map(|(language, _)| *language)
    }

    /// Returns the indexed file of `language`.
    #[must_use]
    pub fn ini(&self, language: Language) -> Option<&GlobalIni<'a>> {
        self.languages
            .iter()
            .find(|(known, _)| *known == language)
            .map(|(_, ini)| ini)
    }

    /// Resolves `key`, with or without its leading `@`, in `language` and, when that has no
    /// non-empty text for it, in `fallback`.
    ///
    /// A [placeholder key](crate::PLACEHOLDER_KEYS) resolves to [`Lookup::Placeholder`] in any
    /// language; a language the set holds no file for has no text for any key.
    #[must_use]
    pub fn lookup(&self, key: &str, language: Language, fallback: Option<Language>) -> Lookup<'a> {
        resolve(key, language, fallback, |key, language| {
            self.text(key, language)
        })
    }

    /// Returns whether `key` has a translation in `language`: [`TextSet::lookup`] without a
    /// fallback finds a text, so an empty text, a placeholder key and a language the set holds no
    /// file for are not translated.
    #[must_use]
    pub fn is_translated(&self, key: &str, language: Language) -> bool {
        matches!(self.lookup(key, language, None), Lookup::Found { .. })
    }

    /// Copies the texts into a set that borrows nothing, to keep after the files are dropped.
    ///
    /// # Panics
    /// Panics if storage cannot be reserved. Use [`Self::try_to_owned_set`] to handle the error.
    #[expect(
        clippy::expect_used,
        reason = "the convenience API returns the owned set directly"
    )]
    #[must_use]
    pub fn to_owned_set(&self) -> OwnedTextSet {
        self.try_to_owned_set()
            .expect("owned localisation storage can be reserved")
    }

    /// Copies the texts into an owned set with fallible storage growth.
    ///
    /// # Errors
    /// Returns a reservation error when owned storage cannot be allocated.
    pub fn try_to_owned_set(&self) -> Result<OwnedTextSet, TryReserveError> {
        OwnedTextSet::try_from_files(
            self.languages
                .iter()
                .map(|(language, ini)| (*language, ini.iter())),
        )
    }

    /// Returns the text of `key` in `language` as the file holds it, also when it is empty;
    /// `None` when the language's file lacks the key or the set holds no file for it.
    #[must_use]
    pub fn text(&self, key: &str, language: Language) -> Option<&'a str> {
        self.ini(language)?.resolve(key)
    }

    /// Returns the language supplying the original spelling emitted for a table key.
    #[must_use]
    pub fn key_language(&self, key: &str) -> Option<Language> {
        self.languages
            .iter()
            .find_map(|(language, ini)| ini.contains_key(key).then_some(*language))
    }

    /// Returns every key any file holds once, ignoring ASCII case, spelled as the first file in
    /// the order of [`Language::ALL`] that holds it spells it, sorted ignoring ASCII case.
    ///
    /// # Panics
    /// Panics if storage cannot be reserved. Use [`Self::try_keys`] to handle the error.
    #[expect(
        clippy::expect_used,
        reason = "the convenience API returns the key vector directly"
    )]
    #[must_use]
    pub fn keys(&self) -> Vec<&'a str> {
        self.try_keys()
            .expect("localisation key storage can be reserved")
    }

    /// Returns the same ordered union as [`Self::keys`] with fallible storage growth.
    ///
    /// # Errors
    /// Returns a reservation error when storage for the keys cannot be allocated.
    pub fn try_keys(&self) -> Result<Vec<&'a str>, TryReserveError> {
        let mut keys: Vec<&'a str> = Vec::new();
        for (position, (_, ini)) in self.languages.iter().enumerate() {
            let earlier = self.languages.get(..position).unwrap_or_default();
            for (key, _) in ini.iter() {
                if !earlier.iter().any(|(_, other)| other.contains_key(key)) {
                    keys.try_reserve(1)?;
                    keys.push(key);
                }
            }
        }
        keys.sort_unstable_by(|a, b| fold_cmp(a, b));
        Ok(keys)
    }
}

fn fold_cmp(a: &str, b: &str) -> Ordering {
    a.bytes()
        .map(|byte| byte.to_ascii_lowercase())
        .cmp(b.bytes().map(|byte| byte.to_ascii_lowercase()))
        .then_with(|| a.cmp(b))
}
