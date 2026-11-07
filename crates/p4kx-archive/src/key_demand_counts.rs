// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use crate::entry_identity::PhysicalEntryId;
use crate::key_demand::KeyDemandSnapshot;

#[derive(Debug, Clone, Copy)]
pub(crate) enum KeyFailure {
    Missing,
    Wrong,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Reasons {
    missing: bool,
    wrong: bool,
}

impl Reasons {
    const fn insert(&mut self, cause: KeyFailure) -> bool {
        let previous = match cause {
            KeyFailure::Missing => std::mem::replace(&mut self.missing, true),
            KeyFailure::Wrong => std::mem::replace(&mut self.wrong, true),
        };
        !previous
    }

    const fn merge(&mut self, other: Self) {
        self.missing |= other.missing;
        self.wrong |= other.wrong;
    }
}

#[derive(Debug, Default)]
pub(crate) struct KeyDemandCounts {
    entries: HashMap<PhysicalEntryId, Reasons>,
    missing: usize,
    wrong: usize,
    first_missing: Option<String>,
    first_wrong: Option<String>,
}

impl KeyDemandCounts {
    pub(crate) fn add(&mut self, entry: &PhysicalEntryId, path: &str, cause: KeyFailure) {
        if !self.entries.entry(entry.clone()).or_default().insert(cause) {
            return;
        }
        let (count, first) = match cause {
            KeyFailure::Missing => (&mut self.missing, &mut self.first_missing),
            KeyFailure::Wrong => (&mut self.wrong, &mut self.first_wrong),
        };
        *count += 1;
        if first.is_none() {
            *first = Some(path.to_owned());
        }
    }

    pub(crate) fn snapshot(&self) -> KeyDemandSnapshot {
        KeyDemandSnapshot {
            missing: self.missing,
            wrong: self.wrong,
            first_missing: self.first_missing.clone(),
            first_wrong: self.first_wrong.clone(),
            distinct_unread: self.entries.len(),
        }
    }

    pub(crate) fn unread_entries(&self) -> UnreadEntries {
        UnreadEntries {
            entries: self.entries.clone(),
        }
    }
}

/// The distinct physical encrypted Entries a [`DemandScope`](crate::DemandScope) saw asked for
/// without a usable key, for counting the Entries of several scopes of one Archive once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnreadEntries {
    entries: HashMap<PhysicalEntryId, Reasons>,
}

impl UnreadEntries {
    /// Adds the Entries of `other`; an Entry in both keeps the causes of both.
    pub fn union(&mut self, other: &Self) {
        for (entry, reasons) in &other.entries {
            self.entries
                .entry(entry.clone())
                .or_default()
                .merge(*reasons);
        }
    }

    /// Returns how many distinct Entries were asked for while no key was set.
    #[must_use]
    pub fn missing(&self) -> usize {
        self.entries
            .values()
            .filter(|reasons| reasons.missing)
            .count()
    }

    /// Returns how many distinct Entries were asked for with a key that does not fit.
    #[must_use]
    pub fn wrong(&self) -> usize {
        self.entries
            .values()
            .filter(|reasons| reasons.wrong)
            .count()
    }

    /// Returns how many distinct Entries could not be read; an Entry under both causes counts once.
    #[must_use]
    pub fn unread(&self) -> usize {
        self.entries.len()
    }
}
