// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;
use std::io::{self, Read};

use p4kx_extraction::{
    AnimationChoice, AnimationSet, ArchiveAnimationRead, ArchivePositionedAnimation,
    ConversionError, Entry, EntryOrigin, ObservedEntryBinding,
};
use p4kx_extraction::{PathIndex, PathIndexBuildError};

use super::{
    ArchiveMaterialReads, ArchiveModels, LayerCache, MAX_ASSEMBLY_VERTICES, ModelFailure,
    TextureVerdicts, read_failure,
};

use crate::entity_rig_catalogue::RigEntry;

const ENTRY_READ_BYTES: usize = 8 << 10;

impl<'archive> ArchiveModels<'archive> {
    /// Builds the provider over an existing index of the same Archive.
    #[must_use]
    pub fn from_path_index(index: PathIndex<'archive>) -> Self {
        Self {
            archive: index.archive(),
            index,
            max_attachment_vertices: MAX_ASSEMBLY_VERTICES,
            layers: LayerCache::new(),
            material_reads: ArchiveMaterialReads::default(),
            observe_nodes: false,
            verdicts: TextureVerdicts::new(),
        }
    }

    pub(crate) fn rig_entry<E>(
        &self,
        path: &str,
        check: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<RigEntry<'archive>, E> {
        for candidate in archive_entry_candidates(path) {
            let entry = self.exact_rig_entry(&candidate, check)?;
            if !matches!(entry, RigEntry::Missing) {
                return Ok(entry);
            }
        }
        Ok(RigEntry::Missing)
    }

    pub(crate) fn exact_rig_entry<E>(
        &self,
        path: &str,
        check: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<RigEntry<'archive>, E> {
        let mut matches = match self.index.matching_entries_with(path, |_| check()) {
            Ok(matches) => matches,
            Err(error) => return lookup_failure(error),
        };
        Ok(match (matches.next(), matches.next()) {
            (None, _) => RigEntry::Missing,
            (Some(Ok(entry)), None) => RigEntry::Unique(entry),
            (Some(Ok(_)), Some(Ok(_))) => RigEntry::Ambiguous,
            (Some(Err(error)), _) | (_, Some(Err(error))) => return lookup_failure(error),
        })
    }

    /// Returns the selected animations with each admitted Archive Entry.
    ///
    /// # Errors
    /// As [`Self::animations`].
    pub fn animations_with_archive_sources(
        &self,
        folders: &[&str],
        choice: &AnimationChoice,
        extra: &[String],
    ) -> Result<ArchiveAnimationRead, ConversionError> {
        let paths = archive_folders(folders);
        let folders: Vec<&str> = paths.iter().map(String::as_str).collect();
        AnimationSet::from_folders_with_archive_sources(&self.index, &folders, choice, extra)
    }

    pub(crate) fn animations_with_clip_evidence(
        &self,
        folders: &[&str],
        choice: &AnimationChoice,
        extra: &[String],
    ) -> Result<ArchiveAnimationRead, ConversionError> {
        let paths = archive_folders(folders);
        let folders: Vec<&str> = paths.iter().map(String::as_str).collect();
        AnimationSet::from_folders_with_clip_evidence(&self.index, &folders, choice, extra)
    }

    pub(crate) fn animations_of(
        &self,
        extra: &[String],
        choice: &AnimationChoice,
    ) -> Result<AnimationSet, ConversionError> {
        AnimationSet::from_folders(&self.index, &[], choice, extra)
    }

    pub(crate) const fn observes_model_sources(&self) -> bool {
        self.observe_nodes
    }

    pub(crate) fn positioned_deploy_clip(
        &self,
        clip: &str,
        extra: &[String],
    ) -> Result<Option<ArchivePositionedAnimation>, ConversionError> {
        ArchivePositionedAnimation::read_first_non_additive(&self.index, clip, extra)
    }

    pub(crate) fn animation_entry_binding(&self, path: &str) -> Option<ObservedEntryBinding> {
        let entry = self.index.find(path)?;
        Some(self.archive.entry_origin(entry)?.binding())
    }

    pub(crate) fn observed_file(
        &self,
        path: &str,
        limit: u64,
    ) -> (Option<EntryOrigin>, Result<Vec<u8>, ModelFailure>) {
        let Some(entry) = self.file_entry(path) else {
            return (None, Err(ModelFailure::Missing));
        };
        let origin = self.archive.entry_origin(entry);
        (origin, self.read_file_entry(entry, limit))
    }

    pub(crate) fn entry_of(&self, path: &str) -> Option<String> {
        archive_entry_candidates(path)
            .into_iter()
            .find_map(|candidate| {
                self.index
                    .find(&candidate)
                    .map(|entry| entry.path().to_owned())
            })
    }

    pub(super) fn file_entry(&self, path: &str) -> Option<&Entry> {
        self.entry_of(path).and_then(|path| self.index.find(&path))
    }

    pub(crate) fn read_file_entry(
        &self,
        entry: &Entry,
        limit: u64,
    ) -> Result<Vec<u8>, ModelFailure> {
        match self.read_file_entry_with(entry, limit, &mut || Ok::<_, Infallible>(())) {
            Ok(result) => result,
            Err(never) => match never {},
        }
    }

    pub(crate) fn read_file_entry_with<E>(
        &self,
        entry: &Entry,
        limit: u64,
        check: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Result<Vec<u8>, ModelFailure>, E> {
        check()?;
        let size = entry.uncompressed_size();
        if size > limit {
            return Ok(Err(ModelFailure::Unreadable {
                reason: format!("the file holds {size} bytes, more than the limit of {limit}"),
            }));
        }
        let opened = self.archive.open_entry(entry).map_err(read_failure);
        check()?;
        let mut reader = match opened {
            Ok(reader) => reader,
            Err(error) => return Ok(Err(error)),
        };
        let mut bytes = Vec::new();
        if let Err(error) = bytes.try_reserve_exact(usize::try_from(size).unwrap_or(usize::MAX)) {
            return Ok(Err(read_failure(error)));
        }
        let mut block = [0; ENTRY_READ_BYTES];
        loop {
            check()?;
            let read = reader.read(&mut block);
            check()?;
            match read {
                Ok(0) => break,
                Ok(count) => {
                    let Some(chunk) = block.get(..count) else {
                        return Ok(Err(read_failure(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Archive reader returned a count larger than its buffer",
                        ))));
                    };
                    bytes.extend_from_slice(chunk);
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Ok(Err(read_failure(error))),
            }
        }
        Ok(Ok(bytes))
    }
}

fn lookup_failure<'archive, E>(error: PathIndexBuildError<E>) -> Result<RigEntry<'archive>, E> {
    match error.into_cause() {
        Ok(interrupted) => Err(interrupted),
        Err(error) => Ok(RigEntry::Unreadable(error.to_string())),
    }
}

/// Returns the model provider's ordered exact archive path forms.
#[must_use]
pub fn archive_entry_candidates(path: &str) -> [String; 2] {
    let path = path.trim_start_matches(['/', '\\']);
    [format!("Data/{path}"), path.to_owned()]
}

fn archive_folders(folders: &[&str]) -> Vec<String> {
    folders
        .iter()
        .map(|folder| format!("Data/{}", folder.trim_start_matches(['/', '\\'])))
        .collect()
}

impl super::ProvidedModel {
    /// Returns the admitted definition that selected this model, if one was read.
    #[must_use]
    pub const fn definition_origin(&self) -> Option<&EntryOrigin> {
        self.definition_origin.as_ref()
    }

    /// Returns the Archive inputs selected for the retained model level.
    #[must_use]
    pub fn archive_sources(&self) -> &[p4kx_extraction::ModelArchiveSource] {
        &self.archive_sources
    }
}
