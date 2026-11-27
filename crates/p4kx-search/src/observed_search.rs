// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForgeOrigin, EntryOrigin, Language, ObservedEntry, TextEncoding};

use crate::{EntryHit, LocalizationHit, SearchReport};

/// Ordered search results retaining only the original inputs that supplied returned hits.
#[derive(Debug, Default)]
pub struct ObservedSearch<'archive> {
    pub(crate) report: SearchReport,
    pub(crate) record_origin: Option<DataForgeOrigin>,
    pub(crate) record_structures: Vec<Option<u32>>,
    pub(crate) localization_origins: Vec<(Language, EntryOrigin)>,
    pub(crate) localization_lines: Vec<u64>,
    pub(crate) entry_origins: Vec<EntryMatchSource<'archive>>,
}

impl<'archive> ObservedSearch<'archive> {
    /// Borrows the existing search result and its original hit ordering.
    #[must_use]
    pub const fn report(&self) -> &SearchReport {
        &self.report
    }

    /// Returns the plain hits and search accounting.
    #[must_use]
    pub fn into_report(self) -> SearchReport {
        self.report
    }

    /// Returns the original structure definition used to name a returned Record’s type.
    #[must_use]
    pub fn record_structure(&self, position: usize) -> Option<u32> {
        self.record_structures.get(position).copied().flatten()
    }

    /// Returns the owner of the DataForge read that produced the Record hits.
    #[must_use]
    pub const fn record_origin(&self) -> Option<&DataForgeOrigin> {
        self.record_origin.as_ref()
    }

    /// Returns the original localization Entry and decoded line coordinate of one returned hit.
    #[must_use]
    pub fn localization_source(&self, position: usize) -> Option<LocalizationMatchSource<'_>> {
        let language = self.report.localization.get(position)?.language;
        let line_index = *self.localization_lines.get(position)?;
        let origin = self
            .localization_origins
            .iter()
            .find(|entry| entry.0 == language)
            .map(|entry| &entry.1);
        Some(LocalizationMatchSource { origin, line_index })
    }

    /// Returns the physical owner and decoding context of one returned Entry hit.
    #[must_use]
    pub fn entry_source(&self, position: usize) -> Option<EntryMatchSource<'archive>> {
        self.entry_origins.get(position).copied()
    }
}

/// Source coordinates of one retained localization line.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct LocalizationMatchSource<'input> {
    /// The exact Entry observed when reading this language.
    pub origin: Option<&'input EntryOrigin>,
    /// The zero-based original INI line position across decoding chunks.
    pub line_index: u64,
}

/// The Entry and text decoding that supplied the matched line.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct EntryMatchSource<'archive> {
    /// The physical Entry actually read by the scanner.
    pub origin: Option<ObservedEntry<'archive>>,
    /// The encoding or transformation that produced the searched text.
    pub encoding: TextEncoding,
}

pub(crate) struct LocatedEntryHit<'archive> {
    pub(crate) hit: EntryHit,
    pub(crate) source: EntryMatchSource<'archive>,
}

pub(crate) struct LocatedLocalizationHit {
    pub(crate) hit: LocalizationHit,
    pub(crate) line_index: u64,
}
