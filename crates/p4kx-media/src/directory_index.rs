// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::{Cell, OnceCell};

use p4kx_extraction::{
    Archive, PathIndex, PathIndexBuildError, PathKeys, TextureMemberIndex, TextureMemberKeys,
};

use crate::lookup_work::LookupWork;
use crate::{CandidateInventory, MediaError, ReferenceUnavailable};

pub(crate) struct DirectoryIndex<'a> {
    pub(crate) archive: &'a Archive,
    keys: OnceCell<PathKeys>,
    members: OnceCell<TextureMemberKeys>,
    live_entries: &'a Cell<u64>,
    registered_entries: Cell<Option<u64>>,
    inventory: Cell<CandidateInventory>,
}

struct Registration {
    entries: u64,
    live_after: u64,
    inventory: CandidateInventory,
}

impl<'a> DirectoryIndex<'a> {
    pub(crate) const fn new(archive: &'a Archive, live_entries: &'a Cell<u64>) -> Self {
        Self {
            archive,
            keys: OnceCell::new(),
            members: OnceCell::new(),
            live_entries,
            registered_entries: Cell::new(None),
            inventory: Cell::new(CandidateInventory::Unavailable(
                ReferenceUnavailable::TargetNotRead,
            )),
        }
    }

    pub(crate) const fn inventory(&self) -> CandidateInventory {
        self.inventory.get()
    }

    pub(crate) fn index(&self, work: &LookupWork<'_>) -> Result<PathIndex<'_>, MediaError> {
        self.initialize(&self.keys, work, || {
            PathKeys::try_new_with(self.archive, |count| work.charge(count)).map_err(error)
        })?;
        self.keys
            .get()
            .map(|keys| keys.index(self.archive))
            .ok_or_else(|| MediaError::Contract("directory keys were not initialized".into()))
    }

    pub(crate) fn texture_members(
        &self,
        work: &LookupWork<'_>,
    ) -> Result<TextureMemberIndex<'_>, MediaError> {
        self.initialize(&self.members, work, || {
            TextureMemberKeys::try_new_with(self.archive, |count| work.charge(count)).map_err(error)
        })?;
        self.members
            .get()
            .map(|keys| keys.index(self.archive))
            .ok_or_else(|| MediaError::Contract("texture member keys were not initialized".into()))
    }

    pub(crate) fn scan_inventory(
        &self,
        work: &LookupWork<'_>,
    ) -> Result<CandidateInventory, MediaError> {
        self.scan_count(work)?;
        if self.registered_entries.get().is_some() {
            return Ok(self.inventory());
        }
        self.inspect_inventory(work)
    }

    fn scan_count(&self, work: &LookupWork<'_>) -> Result<u64, MediaError> {
        let count = u64::try_from(self.archive.entries().len())
            .map_err(|_| MediaError::Contract("source Entry count exceeds u64".into()))?;
        if count > work.source_allowance {
            return Err(MediaError::Limit {
                budget: "sources",
                limit: work.limits.sources,
            });
        }
        Ok(count)
    }

    fn inspect_inventory(&self, work: &LookupWork<'_>) -> Result<CandidateInventory, MediaError> {
        for (_, issue) in self.archive.issues() {
            work.charge(1)?;
            if issue.is_archive_wide() {
                return Ok(CandidateInventory::Unavailable(
                    ReferenceUnavailable::IncompleteInventory,
                ));
            }
        }
        Ok(CandidateInventory::Complete)
    }

    fn initialize<T>(
        &self,
        slot: &OnceCell<T>,
        work: &LookupWork<'_>,
        build: impl FnOnce() -> Result<T, MediaError>,
    ) -> Result<(), MediaError> {
        work.charge(0)?;
        if slot.get().is_some() {
            return Ok(());
        }
        let registration = self.registration(work)?;
        let value = build()?;
        work.charge(0)?;
        slot.set(value)
            .map_err(|_| MediaError::Contract("directory index was already initialized".into()))?;
        if let Some(registration) = registration {
            self.live_entries.set(registration.live_after);
            self.registered_entries.set(Some(registration.entries));
            self.inventory.set(registration.inventory);
        }
        Ok(())
    }

    fn registration(&self, work: &LookupWork<'_>) -> Result<Option<Registration>, MediaError> {
        if self.registered_entries.get().is_some() {
            return Ok(None);
        }
        let count = self.scan_count(work)?;
        let available = work.limits.sources.saturating_sub(self.live_entries.get());
        if count > available {
            return Err(MediaError::Limit {
                budget: "sources",
                limit: work.limits.sources,
            });
        }
        let live_after = self
            .live_entries
            .get()
            .checked_add(count)
            .ok_or_else(|| MediaError::Contract("indexed Entry count exceeds u64".into()))?;
        let inventory = self.inspect_inventory(work)?;
        Ok(Some(Registration {
            entries: count,
            live_after,
            inventory,
        }))
    }
}

impl Drop for DirectoryIndex<'_> {
    fn drop(&mut self) {
        if let Some(count) = self.registered_entries.get() {
            self.live_entries.set(self.live_entries.get() - count);
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
