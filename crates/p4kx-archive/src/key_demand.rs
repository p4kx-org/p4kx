// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::{Arc, Mutex, PoisonError};

use crate::entry_identity::PhysicalEntryId;
use crate::key_demand_counts::{KeyDemandCounts, KeyFailure, UnreadEntries};

/// The encrypted Entries of an Archive that were asked for while no key was set or the one set did
/// not fit.
///
/// A handle shares its record with the [`Archive`](crate::Archive) it came from and with the
/// containers opened from it; clones share it too. The record is never reset, neither by reading
/// nor by setting or clearing the key: it counts every distinct physical Entry since opening.
/// A [`DemandScope`] counts the same events from the moment it is opened.
#[derive(Debug, Clone, Default)]
pub struct KeyDemand {
    record: Arc<Mutex<Record>>,
}

#[derive(Debug, Default)]
struct Record {
    total: KeyDemandCounts,
    next_scope: u64,
    scopes: Vec<(u64, KeyDemandCounts)>,
}

/// What a [`KeyDemand`] recorded at one moment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct KeyDemandSnapshot {
    /// The distinct encrypted Entries that were asked for while no key was set.
    pub missing: usize,
    /// The distinct encrypted Entries that were asked for with a key that does not fit.
    pub wrong: usize,
    /// The path of the first Entry counted in `missing`.
    pub first_missing: Option<String>,
    /// The path of the first Entry counted in `wrong`.
    pub first_wrong: Option<String>,
    pub(crate) distinct_unread: usize,
}

impl KeyDemandSnapshot {
    /// Returns how many distinct encrypted Entries could not be read, for either reason.
    /// An Entry encountered under both causes counts once.
    #[must_use]
    pub const fn unread(&self) -> usize {
        self.distinct_unread
    }
}

impl KeyDemand {
    pub(crate) fn record_missing(&self, entry: &PhysicalEntryId, path: &str) {
        self.record(entry, path, KeyFailure::Missing);
    }

    pub(crate) fn record_wrong(&self, entry: &PhysicalEntryId, path: &str) {
        self.record(entry, path, KeyFailure::Wrong);
    }

    fn record(&self, entry: &PhysicalEntryId, path: &str, cause: KeyFailure) {
        self.lock(|record| {
            record.total.add(entry, path, cause);
            for (_, counts) in &mut record.scopes {
                counts.add(entry, path, cause);
            }
        });
    }

    /// Returns what has been recorded so far.
    #[must_use]
    pub fn snapshot(&self) -> KeyDemandSnapshot {
        self.lock(|record| record.total.snapshot())
    }

    /// Returns whether `other` is a handle to the same record, that is of the same Archive.
    #[must_use]
    pub fn is_same_record(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.record, &other.record)
    }

    /// Opens a [`DemandScope`] that counts from now on, until it is dropped.
    #[must_use]
    pub fn scope(&self) -> DemandScope {
        let id = self.lock(|record| {
            record.next_scope = record.next_scope.saturating_add(1);
            record
                .scopes
                .push((record.next_scope, KeyDemandCounts::default()));
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
        self.with_counts(KeyDemandCounts::snapshot)
    }

    /// Returns the distinct physical Entries recorded since the scope was opened.
    #[must_use]
    pub fn unread(&self) -> UnreadEntries {
        self.with_counts(KeyDemandCounts::unread_entries)
    }

    fn with_counts<T: Default>(&self, read: impl FnOnce(&KeyDemandCounts) -> T) -> T {
        self.demand.lock(|record| {
            record
                .scopes
                .iter()
                .find(|(id, _)| *id == self.id)
                .map_or_default(|(_, counts)| read(counts))
        })
    }
}

impl Drop for DemandScope {
    fn drop(&mut self) {
        self.demand
            .lock(|record| record.scopes.retain(|(id, _)| *id != self.id));
    }
}
