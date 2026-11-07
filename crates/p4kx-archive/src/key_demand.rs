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
/// A [`DemandScope`] counts the same events from the moment it is opened.
#[derive(Debug, Clone, Default)]
pub struct KeyDemand {
    record: Arc<Mutex<Record>>,
}

#[derive(Debug, Default)]
struct Record {
    total: Counts,
    next_scope: u64,
    scopes: Vec<(u64, Counts)>,
}

#[derive(Debug, Default)]
struct Counts {
    missing: Unread,
    wrong: Unread,
}

impl Counts {
    fn snapshot(&self) -> KeyDemandSnapshot {
        KeyDemandSnapshot {
            missing: self.missing.paths.len(),
            wrong: self.wrong.paths.len(),
            first_missing: self.missing.first.clone(),
            first_wrong: self.wrong.first.clone(),
        }
    }
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
        self.record(path, |counts| &mut counts.missing);
    }

    pub(crate) fn record_wrong(&self, path: &str) {
        self.record(path, |counts| &mut counts.wrong);
    }

    /// Adds `path` to the total and to every open scope, in the set `select` picks.
    fn record(&self, path: &str, select: impl Fn(&mut Counts) -> &mut Unread) {
        self.lock(|record| {
            select(&mut record.total).add(path);
            for (_, counts) in &mut record.scopes {
                select(counts).add(path);
            }
        });
    }

    /// Returns what has been recorded so far.
    #[must_use]
    pub fn snapshot(&self) -> KeyDemandSnapshot {
        self.lock(|record| record.total.snapshot())
    }

    /// Opens a [`DemandScope`] that counts from now on, until it is dropped.
    #[must_use]
    pub fn scope(&self) -> DemandScope {
        let id = self.lock(|record| {
            record.next_scope = record.next_scope.saturating_add(1);
            record.scopes.push((record.next_scope, Counts::default()));
            record.next_scope
        });
        DemandScope {
            demand: self.clone(),
            id,
        }
    }

    fn lock<T>(&self, action: impl FnOnce(&mut Record) -> T) -> T {
        action(&mut self.record.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

/// The distinct encrypted Entries asked for without a usable key since it was opened, whoever
/// asked: reads running at the same time on the same Archive count into every open scope.
#[derive(Debug)]
pub struct DemandScope {
    demand: KeyDemand,
    id: u64,
}

impl DemandScope {
    /// Returns what has been recorded since the scope was opened.
    #[must_use]
    pub fn snapshot(&self) -> KeyDemandSnapshot {
        self.demand.lock(|record| {
            record
                .scopes
                .iter()
                .find(|(id, _)| *id == self.id)
                .map(|(_, counts)| counts.snapshot())
                .unwrap_or_default()
        })
    }
}

impl Drop for DemandScope {
    fn drop(&mut self) {
        self.demand
            .lock(|record| record.scopes.retain(|(id, _)| *id != self.id));
    }
}
