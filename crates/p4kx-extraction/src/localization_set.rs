// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use p4kx_archive::{Archive, Entry};
use p4kx_localization::{Language, LocalizationLimits, TextSet};
use rayon::prelude::*;

use crate::localization::{LocalizationEntry, LocalizationLoadError};

/// A `global.ini` Entry whose text was not read cleanly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizationNote {
    path: String,
    lossy: bool,
    skipped: usize,
}

impl LocalizationNote {
    pub(crate) fn for_entry(entry: &LocalizationEntry, skipped: usize) -> Option<Self> {
        let lossy = entry.text().is_lossy();
        (lossy || skipped > 0).then(|| Self {
            path: entry.path().to_owned(),
            lossy,
            skipped,
        })
    }

    /// Returns the path of the Entry.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns whether bytes that are no valid text were replaced by U+FFFD.
    #[must_use]
    pub const fn is_lossy(&self) -> bool {
        self.lossy
    }

    /// Returns the number of non-blank lines that were skipped.
    #[must_use]
    pub const fn skipped(&self) -> usize {
        self.skipped
    }
}

impl fmt::Display for LocalizationNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:", self.path)?;
        if self.lossy {
            write!(f, " invalid bytes replaced")?;
        }
        if self.skipped > 0 {
            let separator = if self.lossy { "," } else { "" };
            let noun = if self.skipped == 1 { "line" } else { "lines" };
            write!(f, "{separator} {} {noun} skipped", self.skipped)?;
        }
        Ok(())
    }
}

/// The `global.ini` Entries of several languages of an Archive, read into memory.
///
/// A language the Archive holds no `global.ini` for is left out, not an error; the Entries are
/// read in parallel.
#[derive(Debug)]
pub struct LocalizationSet {
    entries: Vec<LocalizationEntry>,
}

impl LocalizationSet {
    /// Reads the `global.ini` of every language in `wanted` that `archive` holds.
    ///
    /// # Errors
    ///
    /// Returns the first [`LocalizationLoadError`] of an Entry that cannot be read.
    pub fn read(archive: &Archive, wanted: &[Language]) -> Result<Self, LocalizationLoadError> {
        Self::read_with_limits(archive, wanted, LocalizationLimits::UNLIMITED)
    }

    /// Reads each present wanted language with an independent application of `limits`.
    ///
    /// # Errors
    ///
    /// Returns a [`LocalizationLoadError`] if any selected Entry cannot be read within the
    /// source and decoded byte limits.
    pub fn read_with_limits(
        archive: &Archive,
        wanted: &[Language],
        limits: LocalizationLimits,
    ) -> Result<Self, LocalizationLoadError> {
        if wanted.is_empty() {
            return Ok(Self {
                entries: Vec::new(),
            });
        }

        let located: Vec<(Language, &Entry)> = archive
            .entries()
            .par_iter()
            .filter_map(|entry| {
                let language =
                    Language::from_global_ini_path(entry.path()).filter(|l| wanted.contains(l))?;
                Some((language, entry))
            })
            .collect();
        let mut first_of_each: Vec<(Language, &Entry)> = Vec::with_capacity(located.len());
        for (language, entry) in located {
            if first_of_each.iter().all(|(known, _)| *known != language) {
                first_of_each.push((language, entry));
            }
        }
        first_of_each.sort_by_key(|(language, _)| *language);
        let entries = first_of_each
            .into_par_iter()
            .map(|(language, entry)| {
                LocalizationEntry::read_entry_with_limits(archive, language, entry, limits)
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { entries })
    }

    /// Reads the `global.ini` of every language the Archive holds.
    ///
    /// # Errors
    ///
    /// Returns the first [`LocalizationLoadError`] of an Entry that cannot be read.
    pub fn read_all(archive: &Archive) -> Result<Self, LocalizationLoadError> {
        Self::read(archive, &Language::ALL)
    }

    /// Reads all available supported languages with `limits` applied independently to each Entry.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::read_with_limits`].
    pub fn read_all_with_limits(
        archive: &Archive,
        limits: LocalizationLimits,
    ) -> Result<Self, LocalizationLoadError> {
        Self::read_with_limits(archive, &Language::ALL, limits)
    }

    /// Returns the languages that were read, in the order of [`Language::ALL`].
    pub fn languages(&self) -> impl Iterator<Item = Language> {
        self.entries.iter().map(LocalizationEntry::language)
    }

    /// Returns the Entry that was read for `language`.
    #[must_use]
    pub fn entry(&self, language: Language) -> Option<&LocalizationEntry> {
        self.entries
            .iter()
            .find(|entry| entry.language() == language)
    }

    /// Returns the Entries that were read, in the order of [`Language::ALL`].
    #[must_use]
    pub fn entries(&self) -> &[LocalizationEntry] {
        &self.entries
    }

    /// Returns a note for every Entry that was read lossily or whose index in `texts` skipped
    /// lines, in the order of [`Language::ALL`]; `texts` must come from [`LocalizationSet::index`].
    #[must_use]
    pub fn notes(&self, texts: &TextSet<'_>) -> Vec<LocalizationNote> {
        self.entries
            .iter()
            .filter_map(|entry| {
                let skipped = texts
                    .ini(entry.language())
                    .map_or(0, p4kx_localization::GlobalIni::skipped);
                LocalizationNote::for_entry(entry, skipped)
            })
            .collect()
    }

    /// Indexes every Entry in parallel and returns the set to resolve keys in.
    ///
    /// # Errors
    ///
    /// Returns [`LocalizationLoadError::Index`] if any Entry cannot be indexed within its
    /// retained limits or the index cannot be allocated.
    pub fn index(&self) -> Result<TextSet<'_>, LocalizationLoadError> {
        let files = self
            .entries
            .par_iter()
            .map(|entry| Ok((entry.language(), entry.index()?)))
            .collect::<Result<Vec<_>, LocalizationLoadError>>()?;
        Ok(TextSet::new(files))
    }
}
