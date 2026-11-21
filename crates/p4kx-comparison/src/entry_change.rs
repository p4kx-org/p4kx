// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::Entry;
use crate::path_key::{eq_normalised, top_level};

/// How an Entry differs between the two sides of a Comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// An unpaired occurrence on the new side.
    Added,
    /// An unpaired occurrence on the old side.
    Removed,
    /// A matched occurrence with a different uncompressed size or declared checksum.
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryDecision {
    SizeDiffers,
    ChecksumDiffers,
    OrderedAgainst(usize),
    OppositeExhausted,
    Collision { evidence: usize, ambiguous: bool },
}

/// One Entry that was added, removed or changed.
#[derive(Debug, Clone, Copy)]
pub struct EntryChange<'a> {
    old: Option<&'a Entry>,
    new: Option<&'a Entry>,
    decision: EntryDecision,
}

impl PartialEq for EntryChange<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.old == other.old
            && self.new == other.new
            && self.pairing_is_ambiguous() == other.pairing_is_ambiguous()
    }
}

impl Eq for EntryChange<'_> {}

impl<'a> EntryChange<'a> {
    pub(crate) const fn added(new: &'a Entry, decision: EntryDecision) -> Self {
        Self {
            old: None,
            new: Some(new),
            decision,
        }
    }

    pub(crate) const fn removed(old: &'a Entry, decision: EntryDecision) -> Self {
        Self {
            old: Some(old),
            new: None,
            decision,
        }
    }

    pub(crate) const fn changed(old: &'a Entry, new: &'a Entry, decision: EntryDecision) -> Self {
        Self {
            old: Some(old),
            new: Some(new),
            decision,
        }
    }

    pub(crate) const fn decision(&self) -> EntryDecision {
        self.decision
    }

    /// Returns how the Entry differs.
    #[must_use]
    pub const fn kind(&self) -> ChangeKind {
        match (self.old, self.new) {
            (Some(_), Some(_)) => ChangeKind::Changed,
            (Some(_), None) => ChangeKind::Removed,
            (None, _) => ChangeKind::Added,
        }
    }

    /// Returns whether indistinguishable physical occurrences make this pairing ambiguous.
    #[must_use]
    pub const fn pairing_is_ambiguous(&self) -> bool {
        matches!(
            self.decision,
            EntryDecision::Collision {
                ambiguous: true,
                ..
            }
        )
    }

    /// Returns the Entry path as the new side stores it, or as the old side does for a removed
    /// Entry.
    #[must_use]
    pub fn path(&self) -> &'a str {
        self.new.or(self.old).map_or("", Entry::path)
    }

    /// Returns the Entry of the old side, `None` when it was added.
    #[must_use]
    pub const fn old_entry(&self) -> Option<&'a Entry> {
        self.old
    }

    /// Returns the Entry of the new side, `None` when it was removed.
    #[must_use]
    pub const fn new_entry(&self) -> Option<&'a Entry> {
        self.new
    }
}

/// The number of Entries per change kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, JsonSchema)]
pub struct EntryCounts {
    /// Entries only the new side has.
    pub added: usize,
    /// Entries only the old side has.
    pub removed: usize,
    /// Entries with a different uncompressed size or CRC-32C.
    pub changed: usize,
}

impl EntryCounts {
    pub(crate) const fn count(&mut self, kind: ChangeKind) {
        match kind {
            ChangeKind::Added => self.added += 1,
            ChangeKind::Removed => self.removed += 1,
            ChangeKind::Changed => self.changed += 1,
        }
    }

    /// Returns the number of Entries of every change kind.
    #[must_use]
    pub const fn total(&self) -> usize {
        self.added + self.removed + self.changed
    }
}

/// The counts of the changed Entries below one top-level directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
pub struct DirectoryCounts<'a> {
    /// The top-level directory as the first changed Entry below it spells it; empty for Entries
    /// at the root.
    pub directory: &'a str,
    /// The counts below it.
    #[serde(flatten)]
    pub counts: EntryCounts,
}

/// Counts `changes` per change kind.
pub(crate) fn count(changes: &[EntryChange<'_>]) -> EntryCounts {
    let mut counts = EntryCounts::default();
    for change in changes {
        counts.count(change.kind());
    }
    counts
}

/// Counts `changes` per top-level directory, ignoring case and separator kind, ordered by the
/// normalised directory.
pub(crate) fn count_by_directory<'a>(changes: &[EntryChange<'a>]) -> Vec<DirectoryCounts<'a>> {
    let mut directories: Vec<DirectoryCounts<'a>> = Vec::new();
    for change in changes {
        let directory = top_level(change.path());
        let index = directories
            .iter()
            .rposition(|known| eq_normalised(known.directory, directory))
            .unwrap_or_else(|| {
                directories.push(DirectoryCounts {
                    directory,
                    counts: EntryCounts::default(),
                });
                directories.len() - 1
            });
        if let Some(known) = directories.get_mut(index) {
            known.counts.count(change.kind());
        }
    }
    directories.sort_by(|a, b| crate::path_key::cmp_normalised(a.directory, b.directory));
    directories
}
