// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use schemars::JsonSchema;
use serde::Serialize;

use crate::{BuiltShipBase, CatalogSummary};

/// Whether the Archive key was there for the encrypted Entries an export asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
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
        let demand = archive.key_demand().snapshot();
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
