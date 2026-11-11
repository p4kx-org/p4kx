// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;

use p4kx_extraction::{
    AnimationChoice, AnimationSet, ArchiveAnimationRead, ConversionError, Entry, EntryOrigin,
};

use super::{ArchiveModels, ModelFailure, read_failure};

impl ArchiveModels<'_> {
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
        let folders: Vec<String> = folders
            .iter()
            .map(|folder| format!("Data/{}", folder.trim_start_matches(['/', '\\'])))
            .collect();
        let folders: Vec<&str> = folders.iter().map(String::as_str).collect();
        AnimationSet::from_folders_with_archive_sources(&self.index, &folders, choice, extra)
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

    pub(super) fn file_entry(&self, path: &str) -> Option<&Entry> {
        self.entry_of(path).and_then(|path| self.index.find(&path))
    }

    pub(super) fn read_file_entry(
        &self,
        entry: &Entry,
        limit: u64,
    ) -> Result<Vec<u8>, ModelFailure> {
        let size = entry.uncompressed_size();
        if size > limit {
            return Err(ModelFailure::Unreadable {
                reason: format!("the file holds {size} bytes, more than the limit of {limit}"),
            });
        }
        let reader = self.archive.open_entry(entry).map_err(read_failure)?;
        let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or_default());
        reader
            .take(limit)
            .read_to_end(&mut bytes)
            .map_err(read_failure)?;
        Ok(bytes)
    }
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
