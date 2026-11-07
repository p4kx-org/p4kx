// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::{Cell, OnceCell};

use p4kx_extraction::{Archive, PathIndex, PathIndexBuildError, PathKeys};

use crate::lookup_work::LookupWork;
use crate::{CandidateInventory, MediaError, ReferenceUnavailable};

pub(crate) struct DirectoryIndex<'a> {
    pub(crate) archive: &'a Archive,
    keys: OnceCell<PathKeys>,
    live_keys: &'a Cell<u64>,
    registered_keys: Cell<u64>,
    inventory: Cell<CandidateInventory>,
}

impl<'a> DirectoryIndex<'a> {
    pub(crate) const fn new(archive: &'a Archive, live_keys: &'a Cell<u64>) -> Self {
        Self {
            archive,
            keys: OnceCell::new(),
            live_keys,
            registered_keys: Cell::new(0),
            inventory: Cell::new(CandidateInventory::Unavailable(
                ReferenceUnavailable::TargetNotRead,
            )),
        }
    }

    pub(crate) const fn inventory(&self) -> CandidateInventory {
        self.inventory.get()
    }

    pub(crate) fn index(&self, work: &LookupWork<'_>) -> Result<PathIndex<'_>, MediaError> {
        work.charge(0)?;
        if self.keys.get().is_none() {
            let count = u64::try_from(self.archive.entries().len())
                .map_err(|_| MediaError::Contract("source Entry count exceeds u64".into()))?;
            let available = work.limits.sources.saturating_sub(self.live_keys.get());
            if count > work.source_allowance || count > available {
                return Err(MediaError::Limit {
                    budget: "sources",
                    limit: work.limits.sources,
                });
            }
            let mut inventory = CandidateInventory::Complete;
            for (_, issue) in self.archive.issues() {
                work.charge(1)?;
                if issue.is_archive_wide() {
                    inventory =
                        CandidateInventory::Unavailable(ReferenceUnavailable::IncompleteInventory);
                    break;
                }
            }
            let keys =
                PathKeys::try_new_with(self.archive, |count| work.charge(count)).map_err(error)?;
            self.keys.set(keys).map_err(|_| {
                MediaError::Contract("directory keys were already initialized".into())
            })?;
            self.live_keys.set(self.live_keys.get() + count);
            self.registered_keys.set(count);
            self.inventory.set(inventory);
        }
        self.keys
            .get()
            .map(|keys| keys.index(self.archive))
            .ok_or_else(|| MediaError::Contract("directory keys were not initialized".into()))
    }
}

impl Drop for DirectoryIndex<'_> {
    fn drop(&mut self) {
        if self.keys.get().is_some() {
            self.live_keys
                .set(self.live_keys.get() - self.registered_keys.get());
        }
    }
}

pub(crate) fn error(error: PathIndexBuildError<MediaError>) -> MediaError {
    error
        .into_cause()
        .unwrap_or_else(MediaError::ReferenceIndex)
}

#[cfg(test)]
#[path = "directory_index_tests.rs"]
mod tests;
