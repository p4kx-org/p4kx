// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;
use std::collections::TryReserveError;

use p4kx_extraction::{
    Archive, Language, LocalizationLoadError, LocalizationSet, OwnedTextSet, TextSet, display_text,
};

/// The localisation keys of every language of an Archive with their texts, read once.
///
/// A row is a key, sorted ignoring ASCII case; a column is a language the Archive holds a
/// `global.ini` for. Nothing is copied per row: keys sit in one buffer and texts in the owned
/// [`OwnedTextSet`] of the Archive.
#[derive(Debug)]
pub struct LocalizationTable {
    languages: Vec<Language>,
    keys: String,
    ends: Vec<u32>,
    texts: OwnedTextSet,
    notes: Vec<String>,
}

impl LocalizationTable {
    /// Reads the `global.ini` of every language `archive` holds, indexes them in parallel and
    /// copies the texts into the table.
    ///
    /// # Errors
    ///
    /// Returns a [`TableError`] for unreadable Entries, unrepresentable table coordinates or
    /// failed table reservations.
    pub(super) fn read(archive: &Archive) -> Result<Self, TableError> {
        let set = LocalizationSet::read_all(archive)?;
        let indexed = set.index()?;
        let notes = set
            .notes(&indexed)
            .iter()
            .map(ToString::to_string)
            .collect();
        Self::from_texts(&indexed, notes)
    }

    fn from_texts(indexed: &TextSet<'_>, notes: Vec<String>) -> Result<Self, TableError> {
        let sorted = indexed.try_keys()?;
        validate_row_count(sorted.len())?;
        let mut keys = String::new();
        keys.try_reserve_exact(key_bytes(sorted.iter().map(|key| key.len()))?)?;
        let mut ends = Vec::new();
        ends.try_reserve_exact(sorted.len())?;
        for key in sorted {
            keys.push_str(key);
            ends.push(u32::try_from(keys.len()).map_err(|_| TableError::KeyOffset)?);
        }
        Ok(Self {
            languages: indexed.languages().collect(),
            keys,
            ends,
            texts: indexed.try_to_owned_set()?,
            notes,
        })
    }

    /// Returns the languages the table has a column for, in the order of [`Language::ALL`].
    #[must_use]
    pub fn languages(&self) -> &[Language] {
        &self.languages
    }

    /// Returns the number of keys, which is the number of rows.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.ends.len()
    }

    /// Returns whether the Archive holds no key in any language.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.ends.is_empty()
    }

    /// Returns the key of row `row`, spelled as the first language that holds it spells it.
    #[must_use]
    pub fn key(&self, row: u32) -> Option<&str> {
        let row = usize::try_from(row).ok()?;
        let start = match row.checked_sub(1) {
            Some(before) => usize::try_from(*self.ends.get(before)?).ok()?,
            None => 0,
        };
        let end = usize::try_from(*self.ends.get(row)?).ok()?;
        self.keys.get(start..end)
    }

    /// Returns the text of row `row` in `language` as the file holds it, `\n` still escaped,
    /// or `None` when the key is missing, empty or a placeholder in that language.
    #[must_use]
    pub fn text(&self, row: u32, language: Language) -> Option<&str> {
        let key = self.key(row)?;
        self.texts.lookup(key, language, None).text()
    }

    /// Returns whether row `row` has a translation in `language`.
    #[must_use]
    pub fn is_translated(&self, row: u32, language: Language) -> bool {
        self.key(row)
            .is_some_and(|key| self.texts.is_translated(key, language))
    }

    /// Returns the text of row `row` in `language` with its `\n` escapes turned into line
    /// feeds, as the game shows it.
    #[must_use]
    pub fn decoded_text(&self, row: u32, language: Language) -> Option<String> {
        self.text(row, language)
            .map(|text| display_text(text).into_owned())
    }

    /// Returns what was noticed reading the files: invalid bytes that were replaced and lines
    /// that were skipped, one note per `global.ini`.
    #[must_use]
    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    #[cfg(test)]
    pub(super) fn from_inis(files: &[(Language, &str)]) -> Self {
        let parsed = files.iter().map(|&(language, text)| {
            let ini = p4kx_extraction::GlobalIni::parse(text)
                .unwrap_or_else(|error| unreachable!("valid localisation fixture: {error}"));
            (language, ini)
        });
        Self::from_texts(&TextSet::new(parsed), Vec::new())
            .unwrap_or_else(|error| unreachable!("small localisation table: {error}"))
    }

    pub(super) const fn texts(&self) -> &OwnedTextSet {
        &self.texts
    }

    pub(super) fn position(&self, key: &str) -> Option<u32> {
        let mut low = 0_usize;
        let mut high = self.ends.len();
        while low < high {
            let middle = low + (high - low) / 2;
            let known = self.key(u32::try_from(middle).ok()?)?;
            match fold_cmp(known, key) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return u32::try_from(middle).ok(),
            }
        }
        None
    }
}

fn validate_row_count(count: usize) -> Result<(), TableError> {
    u32::try_from(count.saturating_sub(1)).map_err(|_| TableError::RowIndex { count })?;
    Ok(())
}

fn key_bytes(mut lengths: impl Iterator<Item = usize>) -> Result<usize, TableError> {
    let bytes = lengths
        .try_fold(0, usize::checked_add)
        .ok_or(TableError::KeyOffset)?;
    u32::try_from(bytes).map_err(|_| TableError::KeyOffset)?;
    Ok(bytes)
}

fn fold_cmp(a: &str, b: &str) -> Ordering {
    a.bytes()
        .map(|byte| byte.to_ascii_lowercase())
        .cmp(b.bytes().map(|byte| byte.to_ascii_lowercase()))
}

/// Why the localisation table could not be read.
#[derive(Debug, thiserror::Error)]
pub(super) enum TableError {
    /// A `global.ini` cannot be read or indexed.
    #[error(transparent)]
    Load(#[from] LocalizationLoadError),
    /// A row index cannot fit in the table's representation.
    #[error("the localisation table has {count} rows, exceeding its 32-bit row indices")]
    RowIndex { count: usize },
    /// The combined key bytes cannot fit in the table's offset representation.
    #[error("the localisation key bytes exceed the table's 32-bit offsets")]
    KeyOffset,
    /// The table storage could not be reserved.
    #[error("cannot reserve memory for the localisation table")]
    Reserve(#[from] TryReserveError),
}

#[cfg(test)]
mod tests {
    use super::{TableError, key_bytes, validate_row_count};

    #[test]
    fn table_coordinates_use_their_representable_range() -> Result<(), Box<dyn std::error::Error>> {
        let max = usize::try_from(u32::MAX)?;
        validate_row_count((1 << 20) + 1)?;
        assert_eq!(
            key_bytes([max - 1, 1].into_iter())?,
            max,
            "largest key offset"
        );
        assert!(
            matches!(key_bytes([max, 1].into_iter()), Err(TableError::KeyOffset)),
            "out-of-range key offset remains distinct from a reservation failure"
        );
        if let Some(count) = max.checked_add(1) {
            validate_row_count(count)?;
            assert!(
                matches!(
                    validate_row_count(count + 1),
                    Err(TableError::RowIndex { .. })
                ),
                "the first unrepresentable row index has an explicit error"
            );
        }
        Ok(())
    }
}
