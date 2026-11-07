// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};

/// The encrypted Entries of an Archive that were asked for while no key was set or the one set did
/// not fit.
///
/// A handle shares its record with the [`Archive`](crate::Archive) it came from and with the
/// containers opened from it; clones share it too. The record is never reset, neither by reading
/// nor by setting or clearing the key: it counts every distinct Entry since the Archive was opened.
#[derive(Debug, Clone, Default)]
pub struct KeyDemand {
    record: Arc<Mutex<Record>>,
}

#[derive(Debug, Default)]
struct Record {
    missing: Unread,
    wrong: Unread,
}

#[derive(Debug, Default)]
struct Unread {
    paths: HashSet<String>,
    first: Option<String>,
}

impl Unread {
    fn add(&mut self, path: &str) {
        if self.paths.insert(path.to_owned()) && self.first.is_none() {
            self.first = Some(path.to_owned());
        }
    }
}

/// What a [`KeyDemand`] recorded at one moment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyDemandSnapshot {
    /// The distinct encrypted Entries that were asked for while no key was set.
    pub missing: usize,
    /// The distinct encrypted Entries that were asked for with a key that does not fit.
    pub wrong: usize,
    /// The path of the first Entry counted in `missing`.
    pub first_missing: Option<String>,
    /// The path of the first Entry counted in `wrong`.
    pub first_wrong: Option<String>,
}

impl KeyDemandSnapshot {
    /// Returns how many distinct encrypted Entries could not be read, for either reason.
    #[must_use]
    pub const fn unread(&self) -> usize {
        self.missing.saturating_add(self.wrong)
    }
}

impl KeyDemand {
    pub(crate) fn record_missing(&self, path: &str) {
        self.lock(|record| record.missing.add(path));
    }

    pub(crate) fn record_wrong(&self, path: &str) {
        self.lock(|record| record.wrong.add(path));
    }

    /// Returns what has been recorded so far.
    #[must_use]
    pub fn snapshot(&self) -> KeyDemandSnapshot {
        self.lock(|record| KeyDemandSnapshot {
            missing: record.missing.paths.len(),
            wrong: record.wrong.paths.len(),
            first_missing: record.missing.first.clone(),
            first_wrong: record.wrong.first.clone(),
        })
    }

    fn lock<T>(&self, action: impl FnOnce(&mut Record) -> T) -> T {
        action(&mut self.record.lock().unwrap_or_else(PoisonError::into_inner))
    }
}
