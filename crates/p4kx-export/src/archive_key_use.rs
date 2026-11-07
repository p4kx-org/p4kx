// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, DemandScope, KeyDemandSnapshot, UnreadEntries};
use schemars::JsonSchema;
use serde::Serialize;

use crate::{BuiltShipBase, CatalogSummary};

/// Whether the Archive key was there for the encrypted Entries an export asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub enum ArchiveKeyState {
    /// A key was set and no encrypted Entry failed because it did not fit.
    Given,
    /// No key was set; every encrypted Entry that was asked for is missing from the result.
    Missing,
    /// A key was set and did not fit; the encrypted Entries that were asked for are missing from
    /// the result.
    Wrong,
}

/// How the Archive key served an export, and how many encrypted Entries it could not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveKeyUse {
    state: ArchiveKeyState,
    entries_unread: u64,
}

impl ArchiveKeyUse {
    /// Returns how `archive` served the reads made through it so far.
    ///
    /// The count covers every distinct encrypted Entry that was asked for without a key or with
    /// one that did not fit since the Archive was opened.
    #[must_use]
    pub fn of(archive: &Archive) -> Self {
        Self::from_demand(archive, &archive.key_demand().snapshot())
    }

    /// Returns how `archive` served the reads made while `scope` was open.
    ///
    /// The state is `Given` when a key is set and none of those reads failed because of it, and
    /// `Missing` when no key is set, even if the reads asked for no encrypted Entry.
    #[must_use]
    pub fn scoped(archive: &Archive, scope: &DemandScope) -> Self {
        Self::from_demand(archive, &scope.snapshot())
    }

    /// Returns how `archive` served reads that left `unread` unread, with the state rules of
    /// [`ArchiveKeyUse::scoped`].
    #[must_use]
    pub fn of_unread(archive: &Archive, unread: &UnreadEntries) -> Self {
        let state = if unread.wrong() > 0 {
            ArchiveKeyState::Wrong
        } else if archive.has_key() {
            ArchiveKeyState::Given
        } else {
            ArchiveKeyState::Missing
        };
        Self {
            state,
            entries_unread: unread.unread() as u64,
        }
    }

    /// Returns the use that covers both: the more serious state and the sum of the counts.
    #[must_use]
    pub fn merge(self, other: Self) -> Self {
        Self {
            state: self.state.max(other.state),
            entries_unread: self.entries_unread.saturating_add(other.entries_unread),
        }
    }

    fn from_demand(archive: &Archive, demand: &KeyDemandSnapshot) -> Self {
        let state = if demand.wrong > 0 {
            ArchiveKeyState::Wrong
        } else if archive.has_key() {
            ArchiveKeyState::Given
        } else {
            ArchiveKeyState::Missing
        };
        Self {
            state,
            entries_unread: demand.unread() as u64,
        }
    }

    /// Returns whether the key was given, missing or wrong.
    #[must_use]
    pub const fn state(&self) -> ArchiveKeyState {
        self.state
    }

    /// Returns how many distinct encrypted Entries could not be read, so the result lacks what
    /// they hold.
    #[must_use]
    pub const fn entries_unread(&self) -> u64 {
        self.entries_unread
    }
}

impl CatalogSummary {
    /// Returns whether the Archive key served the export and how many encrypted Entries stayed
    /// unread; the same as the catalog's `source.archiveKey` and `source.encryptedEntriesUnread`.
    #[must_use]
    pub const fn archive_key(&self) -> ArchiveKeyUse {
        self.archive_key
    }
}

impl BuiltShipBase {
    /// Returns whether the Archive key served the export and how many encrypted Entries stayed
    /// unread; the same as the file's `source.archiveKey` and `source.encryptedEntriesUnread`.
    #[must_use]
    pub const fn archive_key(&self) -> ArchiveKeyUse {
        self.archive_key
    }
}
