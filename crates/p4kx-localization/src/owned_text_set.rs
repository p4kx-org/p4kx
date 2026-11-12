// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;

use crate::Language;
use crate::lookup::{Lookup, resolve};

type Rows = Vec<(Box<str>, Box<str>)>;

/// The texts of several languages, owned: a copy of a [`TextSet`](crate::TextSet) that borrows
/// nothing, resolving keys the same way.
#[derive(Debug, Clone, Default)]
pub struct OwnedTextSet {
    languages: Vec<(Language, Rows)>,
}

impl OwnedTextSet {
    pub(crate) fn from_files<'a>(
        files: impl IntoIterator<Item = (Language, Vec<(&'a str, &'a str)>)>,
    ) -> Self {
        let languages = files
            .into_iter()
            .map(|(language, rows)| {
                let mut rows: Rows = rows
                    .into_iter()
                    .map(|(key, text)| (Box::from(key), Box::from(text)))
                    .collect();
                rows.sort_unstable_by(|(a, _), (b, _)| fold_cmp(a, b));
                (language, rows)
            })
            .collect();
        Self { languages }
    }

    /// Returns the languages the set holds a file for, in the order of
    /// [`Language::ALL`](crate::Language::ALL).
    pub fn languages(&self) -> impl Iterator<Item = Language> {
        self.languages.iter().map(|(language, _)| *language)
    }

    /// Returns the number of keys `language` holds.
    #[must_use]
    pub fn len(&self, language: Language) -> usize {
        self.rows(language).map_or(0, <[_]>::len)
    }

    /// Returns whether `language` holds no key or the set holds no file for it.
    #[must_use]
    pub fn is_empty(&self, language: Language) -> bool {
        self.len(language) == 0
    }

    /// Returns every key of `language` with its text, sorted by key ignoring ASCII case; empty
    /// when the set holds no file for the language.
    pub fn iter(&self, language: Language) -> impl Iterator<Item = (&str, &str)> {
        self.rows(language)
            .unwrap_or_default()
            .iter()
            .map(|(key, text)| (&**key, &**text))
    }

    /// Returns the text of `key`, with or without its leading `@`, in `language` as the file
    /// held it, also when it is empty.
    #[must_use]
    pub fn text(&self, key: &str, language: Language) -> Option<&str> {
        let key = key.trim_prefix('@');
        let rows = self.rows(language)?;
        let at = rows
            .binary_search_by(|(known, _)| fold_cmp(known, key))
            .ok()?;
        rows.get(at).map(|(_, text)| &**text)
    }

    /// Resolves `key` like [`TextSet::lookup`](crate::TextSet::lookup).
    #[must_use]
    pub fn lookup(&self, key: &str, language: Language, fallback: Option<Language>) -> Lookup<'_> {
        resolve(key, language, fallback, |key, language| {
            self.text(key, language)
        })
    }

    /// Returns whether `key` has a translation in `language`, like
    /// [`TextSet::is_translated`](crate::TextSet::is_translated).
    #[must_use]
    pub fn is_translated(&self, key: &str, language: Language) -> bool {
        matches!(self.lookup(key, language, None), Lookup::Found { .. })
    }

    fn rows(&self, language: Language) -> Option<&[(Box<str>, Box<str>)]> {
        self.languages
            .iter()
            .find(|(known, _)| *known == language)
            .map(|(_, rows)| rows.as_slice())
    }
}

fn fold_cmp(a: &str, b: &str) -> Ordering {
    a.bytes()
        .map(|byte| byte.to_ascii_lowercase())
        .cmp(b.bytes().map(|byte| byte.to_ascii_lowercase()))
}
