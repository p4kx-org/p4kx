// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::path::Path;
use std::sync::Mutex;

use p4kx_assembly::{LoadoutFileError, LoadoutFileRead, LoadoutFiles};
use p4kx_extraction::{Entry, PathIndex};

use crate::checkpoint::Checkpoint;
use crate::directory_index;
use crate::directory_inputs::DirectoryAdmission;
use crate::input_budget::InputBudget;
use crate::lookup_work::LookupWork;
use crate::source_selection::{EntrySelection, SourceSelection};
use crate::{
    CandidateInventory, ContainerIdentity, Digest, DirectoryLookup, Limits, MediaError,
    MediaWriter, error, source,
};

pub(super) struct EntityLoadoutFiles<'a> {
    index: PathIndex<'a>,
    inventory: CandidateInventory,
    root: &'a Digest,
    chain: &'a [ContainerIdentity],
    selection: SourceSelection<'a>,
    budget: &'a InputBudget,
    admission: DirectoryAdmission<'a>,
    limits: Limits,
    checkpoint: Checkpoint<'a>,
    output_root: &'a Path,
    first_failure: Mutex<Option<MediaError>>,
}

impl<'a> EntityLoadoutFiles<'a> {
    pub(super) fn new(
        lookup: &'a DirectoryLookup<'_>,
        writer: &'a MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let limits = writer.limits();
        let inputs = lookup.input_state()?;
        let index = lookup.index.index(&LookupWork {
            limits,
            checkpoint: writer.checkpoint,
            budget: lookup.budget,
            source_allowance: lookup.allowance(limits.sources)?,
        })?;
        Ok(Self {
            index,
            inventory: lookup.index.inventory(),
            root: lookup.root,
            chain: lookup.chain,
            selection: lookup.selection,
            budget: inputs.budget,
            admission: inputs.admission(),
            limits,
            checkpoint: writer.checkpoint,
            output_root: &writer.root,
            first_failure: Mutex::new(None),
        })
    }

    pub(super) fn check(&self) -> Result<(), MediaError> {
        self.checkpoint.check()?;
        let mut failure = self.first_failure.lock().map_err(|_| {
            MediaError::Contract("animation Loadout input state was interrupted".into())
        })?;
        failure.take().map_or(Ok(()), Err)
    }

    fn select(&self, requested: &str) -> Result<Option<&Entry>, MediaError> {
        let path =
            archive_path(requested).ok_or_else(|| unavailable(requested, "invalid XML path"))?;
        if !matches!(self.inventory, CandidateInventory::Complete) {
            return Err(unavailable(&path, "incomplete archive inventory"));
        }
        let charge = |count| self.budget.metadata(0, count, self.limits, self.checkpoint);
        charge(1)?;
        let mut matching = self
            .index
            .matching_entries_with(&path, charge)
            .map_err(directory_index::error)?;
        let Some(entry) = matching
            .next()
            .transpose()
            .map_err(directory_index::error)?
        else {
            return Ok(None);
        };
        if matching
            .next()
            .transpose()
            .map_err(directory_index::error)?
            .is_some()
        {
            return Err(unavailable(&path, "ambiguous physical Entries"));
        }
        if self.selection.classify(self.chain, entry.path()) == EntrySelection::Excluded {
            return Err(unavailable(entry.path(), "excluded by the Archive filter"));
        }
        if self.index.archive().raw_path(entry.index()).is_some() {
            return Err(unavailable(entry.path(), "unverified Archive path bytes"));
        }
        Ok(Some(entry))
    }

    fn read_entry(&self, entry: &Entry) -> Result<LoadoutFileRead, MediaError> {
        let archive = self.index.archive();
        let declared = source::entry(self.root, archive, entry, self.chain)?;
        let result = (|| {
            let size = entry.uncompressed_size();
            if size > self.limits.entry_bytes {
                return Err(MediaError::Limit {
                    budget: "entry_bytes",
                    limit: self.limits.entry_bytes,
                });
            }
            self.admission.admit(
                entry.index(),
                self.limits,
                self.checkpoint,
                self.output_root,
            )?;
            self.budget.bytes(size, self.limits)?;
            let file =
                LoadoutFileRead::read_archive(archive, entry, &|| self.checkpoint.check().is_err());
            self.checkpoint.check()?;
            if let Err(cause) = file.outcome() {
                return Err(error::io(
                    Path::new(entry.path()),
                    std::io::Error::other(cause.clone()),
                ));
            }
            Ok(file)
        })();
        result.map_err(|cause| error::at(&declared, cause))
    }

    fn remember(&self, path: &str, error: MediaError) -> LoadoutFileRead {
        let reason = error.to_string();
        if let Ok(mut first) = self.first_failure.lock() {
            first.get_or_insert(error);
        }
        LoadoutFileRead::unbound(path, Err(LoadoutFileError::Unreadable { reason }))
    }
}

impl LoadoutFiles for EntityLoadoutFiles<'_> {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        self.read_observed(path).into_result()
    }

    fn read_observed(&self, path: &str) -> LoadoutFileRead {
        if self
            .first_failure
            .lock()
            .map_or(true, |failure| failure.is_some())
        {
            return LoadoutFileRead::unbound(path, Err(LoadoutFileError::ObservationStopped));
        }
        match self
            .select(path)
            .and_then(|entry| entry.map(|entry| self.read_entry(entry)).transpose())
        {
            Ok(Some(file)) => file,
            Ok(None) => LoadoutFileRead::unbound(path, Err(LoadoutFileError::NotFound)),
            Err(cause) => self.remember(path, cause),
        }
    }
}

fn archive_path(path: &str) -> Option<Cow<'_, str>> {
    let path = path.trim_start_matches(['/', '\\']);
    if path.contains([':', '\0', '!'])
        || path
            .split(['/', '\\'])
            .any(|part| matches!(part, "" | "." | ".."))
        || !path
            .rsplit_once('.')
            .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("xml"))
    {
        return None;
    }
    if path.as_bytes().get(..5).is_some_and(|prefix| {
        prefix.eq_ignore_ascii_case(b"Data/") || prefix.eq_ignore_ascii_case(b"Data\\")
    }) {
        Some(Cow::Borrowed(path))
    } else {
        Some(Cow::Owned(format!("Data/{path}")))
    }
}

fn unavailable(path: &str, reason: &str) -> MediaError {
    MediaError::Contract(format!("animation XML Loadout input {path}: {reason}"))
}
