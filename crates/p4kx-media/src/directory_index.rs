// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::{Cell, OnceCell};
use std::collections::{HashMap, TryReserveError};

use p4kx_extraction::{
    Archive, Entry, PathIndex, PathIndexBuildError, PathKeys, TextureMemberIndex, TextureMemberKeys,
};

use p4kx_extraction::sound::wwise_path;

use crate::lookup_work::LookupWork;
use crate::{CandidateInventory, MediaError, ReferenceUnavailable};

pub(crate) struct DirectoryIndex<'a> {
    pub(crate) archive: &'a Archive,
    pub(crate) foreign_banks: crate::audio_foreign::Scope,
    keys: OnceCell<PathKeys>,
    members: OnceCell<TextureMemberKeys>,
    loose_wems: OnceCell<LooseWemKeys>,
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
    pub(crate) fn new(archive: &'a Archive, live_entries: &'a Cell<u64>) -> Self {
        Self {
            archive,
            foreign_banks: crate::audio_foreign::Scope::default(),
            keys: OnceCell::new(),
            members: OnceCell::new(),
            loose_wems: OnceCell::new(),
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

    pub(crate) fn loose_wems(
        &self,
        media_id: u32,
        work: &LookupWork<'_>,
    ) -> Result<LooseWemMatches<'_>, MediaError> {
        self.initialize(&self.loose_wems, work, || {
            LooseWemKeys::read(self.archive, work)
        })?;
        let keys = self
            .loose_wems
            .get()
            .ok_or_else(|| MediaError::Contract("loose WEM keys were not initialized".into()))?;
        let positions = keys.positions.get(&media_id).map_or(&[][..], Vec::as_slice);
        work.charge(1 + positions.len() as u64)?;
        Ok(LooseWemMatches {
            archive: self.archive,
            positions,
            inventory: self.inventory(),
        })
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

struct LooseWemKeys {
    positions: HashMap<u32, Vec<usize>>,
}

impl LooseWemKeys {
    fn read(archive: &Archive, work: &LookupWork<'_>) -> Result<Self, MediaError> {
        let mut positions = HashMap::<u32, Vec<usize>>::new();
        for (position, entry) in archive.entries().iter().enumerate() {
            work.charge(1)?;
            let Some(name) = wwise_path::loose_name(entry.path()) else {
                continue;
            };
            positions
                .try_reserve(1)
                .map_err(|error| loose_allocation(&error))?;
            let group = positions.entry(name.id).or_default();
            group
                .try_reserve(1)
                .map_err(|error| loose_allocation(&error))?;
            group.push(position);
        }
        Ok(Self { positions })
    }
}

fn loose_allocation(error: &TryReserveError) -> MediaError {
    MediaError::Contract(format!("cannot allocate the loose WEM index: {error}"))
}

pub(crate) struct LooseWemMatches<'scope> {
    pub(crate) archive: &'scope Archive,
    positions: &'scope [usize],
    pub(crate) inventory: CandidateInventory,
}

impl<'scope> LooseWemMatches<'scope> {
    pub(crate) const fn count(&self) -> usize {
        self.positions.len()
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = Result<&'scope Entry, MediaError>> + '_ {
        self.positions.iter().map(|&position| {
            self.archive
                .entries()
                .get(position)
                .ok_or_else(|| MediaError::Contract("indexed loose WEM Entry disappeared".into()))
        })
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
