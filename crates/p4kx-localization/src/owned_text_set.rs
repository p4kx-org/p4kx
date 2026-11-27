// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;
use std::collections::TryReserveError;

use crate::Language;
use crate::lookup::{Lookup, resolve};

/// The texts of several languages, owned: a copy of a [`TextSet`](crate::TextSet) that borrows
/// nothing, resolving keys the same way.
#[derive(Debug, Clone, Default)]
pub struct OwnedTextSet {
    languages: Vec<LanguageTexts>,
}

impl OwnedTextSet {
    pub(crate) fn try_from_files<'a, Rows>(
        files: impl IntoIterator<Item = (Language, Rows)>,
    ) -> Result<Self, TryReserveError>
    where
        Rows: IntoIterator<Item = (&'a str, &'a str)>,
    {
        let mut languages = Vec::new();
        for (language, rows) in files {
            languages.try_reserve(1)?;
            languages.push(LanguageTexts::read(language, rows)?);
        }
        Ok(Self { languages })
    }

    /// Returns the languages the set holds a file for, in the order of
    /// [`Language::ALL`](crate::Language::ALL).
    pub fn languages(&self) -> impl Iterator<Item = Language> {
        self.languages.iter().map(|file| file.language)
    }

    /// Returns the number of keys `language` holds.
    #[must_use]
    pub fn len(&self, language: Language) -> usize {
        self.file(language).map_or(0, |file| file.rows.len())
    }

    /// Returns whether `language` holds no key or the set holds no file for it.
    #[must_use]
    pub fn is_empty(&self, language: Language) -> bool {
        self.len(language) == 0
    }

    /// Returns every key of `language` with its text, sorted by key ignoring ASCII case; empty
    /// when the set holds no file for the language.
    pub fn iter(&self, language: Language) -> impl Iterator<Item = (&str, &str)> {
        self.file(language)
            .into_iter()
            .flat_map(LanguageTexts::iter)
    }

    /// Returns the text of `key`, with or without its leading `@`, in `language` as the file
    /// held it, also when it is empty.
    #[must_use]
    pub fn text(&self, key: &str, language: Language) -> Option<&str> {
        let key = key.trim_prefix('@');
        self.file(language)?.text(key)
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

    fn file(&self, language: Language) -> Option<&LanguageTexts> {
        self.languages.iter().find(|file| file.language == language)
    }
}

#[derive(Debug, Clone)]
struct LanguageTexts {
    language: Language,
    contents: String,
    rows: Vec<TextRow>,
}

impl LanguageTexts {
    fn read<'a>(
        language: Language,
        texts: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<Self, TryReserveError> {
        let mut contents = String::new();
        let mut rows = Vec::new();
        for (key, text) in texts {
            rows.try_reserve(1)?;
            let key_start = append(&mut contents, key)?;
            let text_start = append(&mut contents, text)?;
            rows.push(TextRow {
                key_start,
                text_start,
                end: contents.len(),
            });
        }
        rows.sort_unstable_by(|first, second| {
            fold_cmp(first.key(&contents), second.key(&contents))
        });
        Ok(Self {
            language,
            contents,
            rows,
        })
    }

    fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.rows
            .iter()
            .map(|row| (row.key(&self.contents), row.text(&self.contents)))
    }

    fn text(&self, key: &str) -> Option<&str> {
        let index = self
            .rows
            .binary_search_by(|row| fold_cmp(row.key(&self.contents), key))
            .ok()?;
        self.rows.get(index).map(|row| row.text(&self.contents))
    }
}

/// Adjacent key and text ranges in the same UTF-8 buffer.
#[derive(Debug, Clone)]
struct TextRow {
    key_start: usize,
    text_start: usize,
    end: usize,
}

impl TextRow {
    fn key<'a>(&self, contents: &'a str) -> &'a str {
        &contents[self.key_start..self.text_start]
    }

    fn text<'a>(&self, contents: &'a str) -> &'a str {
        &contents[self.text_start..self.end]
    }
}

fn append(contents: &mut String, text: &str) -> Result<usize, TryReserveError> {
    let start = contents.len();
    contents.try_reserve(text.len())?;
    contents.push_str(text);
    Ok(start)
}

fn fold_cmp(a: &str, b: &str) -> Ordering {
    a.bytes()
        .map(|byte| byte.to_ascii_lowercase())
        .cmp(b.bytes().map(|byte| byte.to_ascii_lowercase()))
}
