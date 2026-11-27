// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::Entry;
use crate::entry_change::EntryDecision;

/// Why a matched Entry pair lacks comparable directory checksums.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum IncomparableReason {
    /// The declared checksums use different algorithms.
    ChecksumAlgorithmsDiffer,
}

/// A matched Entry pair whose directory metadata cannot establish a content change.
#[derive(Debug, Clone, Copy)]
pub struct IncomparableEntry<'archive> {
    old: &'archive Entry,
    new: &'archive Entry,
    decision: EntryDecision,
}

impl PartialEq for IncomparableEntry<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.old == other.old
            && self.new == other.new
            && self.pairing_is_ambiguous() == other.pairing_is_ambiguous()
    }
}

impl Eq for IncomparableEntry<'_> {}

impl<'archive> IncomparableEntry<'archive> {
    pub(crate) const fn new(
        old: &'archive Entry,
        new: &'archive Entry,
        decision: EntryDecision,
    ) -> Self {
        Self { old, new, decision }
    }

    /// Returns why the directory comparison could not decide the pair.
    #[must_use]
    pub const fn reason(&self) -> IncomparableReason {
        IncomparableReason::ChecksumAlgorithmsDiffer
    }

    /// Returns whether indistinguishable physical occurrences make the pairing ambiguous.
    #[must_use]
    pub const fn pairing_is_ambiguous(&self) -> bool {
        matches!(
            self.decision,
            EntryDecision::Incomparable {
                ambiguous: true,
                ..
            }
        )
    }

    /// Returns the original path on the new side.
    #[must_use]
    pub fn path(&self) -> &'archive str {
        self.new.path()
    }

    /// Returns the matched old Entry.
    #[must_use]
    pub const fn old_entry(&self) -> &'archive Entry {
        self.old
    }

    /// Returns the matched new Entry.
    #[must_use]
    pub const fn new_entry(&self) -> &'archive Entry {
        self.new
    }

    pub(crate) const fn decision(&self) -> EntryDecision {
        self.decision
    }
}
