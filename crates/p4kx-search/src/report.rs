// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, Language};

use crate::{Excerpt, Source, Sources};

/// A match in a string, localisation key or enum value of a Record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordHit {
    /// The index of the Record in definition order.
    pub record_index: u32,
    /// The GUID of the Record.
    pub guid: Guid,
    /// The record path, without `.xml`.
    pub record_path: String,
    /// The Record name, e.g. `EntityClassDefinition.AEGS_Gladius`.
    pub name: String,
    /// The name of the Record's struct.
    pub type_name: String,
    /// The path of the field with the index of every array element on the way, e.g.
    /// `Components[3].AttachDef.Manufacturer`.
    pub field: String,
    /// The text around the match.
    pub excerpt: Excerpt,
}

/// A match in a localisation text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizationHit {
    /// The language of the `global.ini`.
    pub language: Language,
    /// The key as the line spells it, with a comma suffix if it has one.
    pub key: String,
    /// The text around the match.
    pub excerpt: Excerpt,
}

/// A match in a line of an Entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryHit {
    /// The path of the Entry in the Archive.
    pub path: String,
    /// The number of the line, from 1; `CryXml` counts the lines of its XML form.
    pub line: u64,
    /// The offset in bytes of the start of the line in the UTF-8 text of the Entry, without a byte
    /// order mark; for CryXml in its XML form.
    pub offset: u64,
    /// The text around the match.
    pub excerpt: Excerpt,
}

/// What a search scanned and what it left out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct SearchStats {
    /// The Records whose tree was read.
    pub records_scanned: u64,
    /// The Records whose tree could not be built; they are skipped.
    pub records_unreadable: u64,
    /// The localisation texts looked at.
    pub texts_scanned: u64,
    /// The languages whose `global.ini` was read.
    pub languages_scanned: u32,
    /// The languages whose `global.ini` could not be read.
    pub languages_unreadable: u32,
    /// The Entries whose text was read.
    pub entries_scanned: u64,
    /// The Entries skipped because they hold binary data.
    pub entries_binary: u64,
    /// The Entries skipped because they are larger than the limit.
    pub entries_too_large: u64,
    /// The encrypted Entries skipped because the Archive has no key or a wrong one.
    pub entries_key_required: u64,
    /// The Entries skipped because their data could not be read.
    pub entries_unreadable: u64,
}

impl SearchStats {
    /// Returns the number of Entries that were not searched.
    #[must_use]
    pub const fn entries_skipped(&self) -> u64 {
        self.entries_binary
            + self.entries_too_large
            + self.entries_key_required
            + self.entries_unreadable
    }

    pub(crate) const fn merge(&mut self, other: &Self) {
        self.records_scanned += other.records_scanned;
        self.records_unreadable += other.records_unreadable;
        self.texts_scanned += other.texts_scanned;
        self.languages_scanned += other.languages_scanned;
        self.languages_unreadable += other.languages_unreadable;
        self.entries_scanned += other.entries_scanned;
        self.entries_binary += other.entries_binary;
        self.entries_too_large += other.entries_too_large;
        self.entries_key_required += other.entries_key_required;
        self.entries_unreadable += other.entries_unreadable;
    }
}

/// A source that was asked for but has nothing to search in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unavailable {
    /// The source.
    pub source: Source,
    /// Why, in words.
    pub reason: String,
}

/// The hits of one source, in the order of the source, and whether more exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found<T> {
    /// The hits, at most the limit.
    pub hits: Vec<T>,
    /// Whether the source holds more matches than the limit.
    pub has_more: bool,
    /// What the source scanned and left out.
    pub stats: SearchStats,
}

impl<T> Found<T> {
    pub(crate) fn map_hits<U>(self, project: impl FnMut(T) -> U) -> Found<U> {
        Found {
            hits: self.hits.into_iter().map(project).collect(),
            has_more: self.has_more,
            stats: self.stats,
        }
    }
}

/// The result of a search over several sources.
///
/// Hits are in the order of their source: Records in Record order, localisation by language and
/// line, Entries in path order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchReport {
    /// The matches in Records.
    pub records: Vec<RecordHit>,
    /// The matches in localisation texts.
    pub localization: Vec<LocalizationHit>,
    /// The matches in Entries.
    pub entries: Vec<EntryHit>,
    /// The sources that hold more matches than the limit.
    pub has_more: Sources,
    /// The sources that had nothing to search in.
    pub unavailable: Vec<Unavailable>,
    /// What was scanned and left out.
    pub stats: SearchStats,
}

impl SearchReport {
    /// Returns the number of hits over all sources.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.records.len() + self.localization.len() + self.entries.len()
    }

    /// Returns whether there is no hit.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
