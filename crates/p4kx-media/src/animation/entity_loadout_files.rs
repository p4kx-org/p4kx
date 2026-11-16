// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::path::Path;
use std::sync::Mutex;

use p4kx_assembly::{LoadoutFileError, LoadoutFileRead, LoadoutFiles, MAX_XML_LOADOUT_LEN};
use p4kx_extraction::{Entry, PathIndex};

use crate::checkpoint::Checkpoint;
use crate::directory_index;
use crate::directory_inputs::DirectoryAdmission;
use crate::input_budget::InputBudget;
use crate::lookup_work::LookupWork;
use crate::required_reader::{self, RequiredReader};
use crate::source_selection::{EntrySelection, SourceSelection};
use crate::{
    CandidateInventory, ContainerIdentity, Digest, DirectoryLookup, Limits, MediaError,
    MediaWriter, error, source,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntityLoadoutMode {
    RetainedAnimations,
    ScopedDefinitions,
}

enum Selection<'entry> {
    Entry(&'entry Entry),
    Refused(LoadoutFileError),
}

pub(super) struct EntityLoadoutFiles<'a> {
    mode: EntityLoadoutMode,
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
        mode: EntityLoadoutMode,
        lookup: &'a DirectoryLookup<'_>,
        writer: &'a MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        if mode == EntityLoadoutMode::RetainedAnimations {
            writer.require_output_callback()?;
            writer.require_source(&lookup.owning_source(0))?;
        }
        let limits = writer.limits();
        let inputs = lookup.input_state()?;
        let index = lookup.index.index(&LookupWork {
            limits,
            checkpoint: writer.checkpoint,
            budget: lookup.budget,
            source_allowance: lookup.allowance(limits.sources)?,
        })?;
        Ok(Self {
            mode,
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

    fn select(&self, requested: &str) -> Result<Selection<'_>, MediaError> {
        let Some(path) = archive_path(requested) else {
            return self.refused(
                requested,
                "invalid XML path",
                LoadoutFileError::UnprovedSelection,
            );
        };
        if !matches!(self.inventory, CandidateInventory::Complete) {
            return self.refused(
                &path,
                "incomplete archive inventory",
                LoadoutFileError::UnprovedSelection,
            );
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
            return Ok(Selection::Refused(LoadoutFileError::NotFound));
        };
        if matching
            .next()
            .transpose()
            .map_err(directory_index::error)?
            .is_some()
        {
            return self.refused(
                &path,
                "ambiguous physical Entries",
                LoadoutFileError::Ambiguous,
            );
        }
        self.selected(entry)
    }

    fn selected<'entry>(&self, entry: &'entry Entry) -> Result<Selection<'entry>, MediaError> {
        let classified = self.selection.classify(self.chain, entry.path());
        match (self.mode, classified) {
            (EntityLoadoutMode::ScopedDefinitions, EntrySelection::Excluded) => {
                return Err(unavailable(entry.path(), "excluded by the Archive filter"));
            }
            (EntityLoadoutMode::RetainedAnimations, _)
                if classified != EntrySelection::Included =>
            {
                return Ok(Selection::Refused(LoadoutFileError::NotSelected));
            }
            _ => {}
        }
        if self.index.archive().raw_path(entry.index()).is_some() {
            return self.refused(
                entry.path(),
                "unverified Archive path bytes",
                LoadoutFileError::UnprovedSelection,
            );
        }
        Ok(Selection::Entry(entry))
    }

    fn refused<'entry>(
        &self,
        path: &str,
        reason: &str,
        failure: LoadoutFileError,
    ) -> Result<Selection<'entry>, MediaError> {
        match self.mode {
            EntityLoadoutMode::ScopedDefinitions => Err(unavailable(path, reason)),
            EntityLoadoutMode::RetainedAnimations => Ok(Selection::Refused(failure)),
        }
    }

    fn read_entry(&self, entry: &Entry) -> Result<LoadoutFileRead, MediaError> {
        let archive = self.index.archive();
        let declared = source::entry(self.root, archive, entry, self.chain)?;
        let result = match self.mode {
            EntityLoadoutMode::RetainedAnimations => self.read_retained(entry),
            EntityLoadoutMode::ScopedDefinitions => self.read_definition_input(entry),
        };
        result.map_err(|cause| error::at(&declared, cause))
    }

    fn read_definition_input(&self, entry: &Entry) -> Result<LoadoutFileRead, MediaError> {
        let archive = self.index.archive();
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
    }

    fn read_retained(&self, entry: &Entry) -> Result<LoadoutFileRead, MediaError> {
        let size = entry.uncompressed_size();
        if size > MAX_XML_LOADOUT_LEN {
            return Ok(LoadoutFileRead::unbound(
                entry.path(),
                Err(LoadoutFileError::TooLarge { size }),
            ));
        }
        let retained = self
            .admission
            .completed_payload(entry.index(), self.checkpoint)?
            .ok_or_else(|| {
                MediaError::Contract(
                    "selected XML source has no completed retention receipt".into(),
                )
            })?;
        let proof = retained.original.as_deref().ok_or_else(|| {
            MediaError::Contract("selected XML source has no original decoded payload proof".into())
        })?;
        let mut reader = RequiredReader::open(
            &self.output_root.join(retained.raw.path),
            entry.path(),
            size,
            self.budget,
            self.checkpoint,
            self.limits,
        )?;
        let bytes = reader.read_payload()?;
        let replay =
            LoadoutFileRead::from_retained(self.index.archive(), entry, proof, bytes, &|| {
                self.checkpoint.check().is_err()
            });
        if matches!(replay, Err(LoadoutFileError::ObservationStopped)) {
            self.checkpoint.check()?;
        }
        let replay = replay
            .map_err(|cause| error::io(Path::new(entry.path()), std::io::Error::other(cause)));
        required_reader::checked(replay, self.checkpoint)
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

    fn reads_vehicle_implementations(&self) -> bool {
        self.mode == EntityLoadoutMode::ScopedDefinitions
    }

    fn read_observed(&self, path: &str) -> LoadoutFileRead {
        if self
            .first_failure
            .lock()
            .map_or(true, |failure| failure.is_some())
        {
            return LoadoutFileRead::unbound(path, Err(LoadoutFileError::ObservationStopped));
        }
        match self.select(path).and_then(|selection| match selection {
            Selection::Entry(entry) => self.read_entry(entry),
            Selection::Refused(cause) => Ok(LoadoutFileRead::unbound(path, Err(cause))),
        }) {
            Ok(file) => file,
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
