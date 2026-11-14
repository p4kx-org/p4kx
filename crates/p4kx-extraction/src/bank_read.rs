// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io;
use std::path::Path;

use p4kx_archive::{Archive, Entry, EntryOrigin};

use crate::entry_content::find_entry;
use crate::output_file::read_error;
use crate::sound::read_capped;
use crate::source_protection::SourceProtection;
use crate::source_scope::SourceInput;
use crate::{ConversionError, FileStamp, ObservedFileOrigin, UnknownEntries};

use super::{Bank, MAX_SOUNDBANK_LEN};

#[derive(Debug)]
pub(super) enum BankReadOrigin {
    Unobserved,
    Archive(EntryOrigin),
    File(ObservedFileOrigin),
}

enum FileObservation {
    Unobserved,
    Observed,
}

impl Bank {
    /// Reads and parses the soundbank Entry at `path`, matched with either separator and ignoring
    /// ASCII case.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::UnknownEntry`] when no Entry has that path,
    /// [`ConversionError::SoundTooLarge`] when it exceeds [`MAX_SOUNDBANK_LEN`],
    /// [`ConversionError::Soundbank`] when it is no valid soundbank, and another
    /// [`ConversionError`] when it cannot be read.
    pub fn from_archive(archive: &Archive, path: &str) -> Result<Self, ConversionError> {
        let entry =
            find_entry(archive, path).ok_or_else(|| UnknownEntries(vec![path.to_owned()]))?;
        Self::read_entry(archive, entry, BankReadOrigin::Unobserved)
    }

    /// Reads the matched Entry and retains its physical source observation.
    ///
    /// # Errors
    /// Returns the same read and format errors as `Self::from_archive`.
    pub fn from_archive_observed(archive: &Archive, path: &str) -> Result<Self, ConversionError> {
        let entry =
            find_entry(archive, path).ok_or_else(|| UnknownEntries(vec![path.to_owned()]))?;
        Self::read_observed_entry(archive, entry)
    }

    /// Reads the exact Archive Entry position and retains its original physical source.
    ///
    /// # Errors
    /// Returns the read and format errors of [`Self::from_archive`].
    pub fn from_archive_position_observed(
        archive: &Archive,
        position: usize,
    ) -> Result<Self, ConversionError> {
        let entry = archive
            .entries()
            .get(position)
            .ok_or_else(|| UnknownEntries(vec![format!("Entry position {position}")]))?;
        Self::read_observed_entry(archive, entry)
    }

    fn read_observed_entry(archive: &Archive, entry: &Entry) -> Result<Self, ConversionError> {
        let origin = archive
            .entry_origin(entry)
            .map_or(BankReadOrigin::Unobserved, BankReadOrigin::Archive);
        Self::read_entry(archive, entry, origin)
    }

    fn read_entry(
        archive: &Archive,
        entry: &Entry,
        origin: BankReadOrigin,
    ) -> Result<Self, ConversionError> {
        let protection = SourceProtection::new(Some(archive.path()));
        let name = entry.path().to_owned();
        let reader = archive.open_entry(entry)?;
        let bytes = read_capped(reader, entry.uncompressed_size(), MAX_SOUNDBANK_LEN).map_err(
            |source| ConversionError::ReadEntry {
                entry: name.clone(),
                source,
            },
        )?;
        Self::parse(name, bytes, protection, origin)
    }

    /// Reads and parses the soundbank file at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::NoSuchFile`] when the file does not exist,
    /// [`ConversionError::SoundTooLarge`] when it exceeds [`MAX_SOUNDBANK_LEN`],
    /// [`ConversionError::Soundbank`] when it is no valid soundbank, and another
    /// [`ConversionError`] when it cannot be read.
    pub fn from_file(path: &Path) -> Result<Self, ConversionError> {
        Self::read_file(path, FileObservation::Unobserved)
    }

    /// Reads a standalone bank and observes the original opened file.
    ///
    /// # Errors
    /// Returns a read or format failure, including file observation I/O errors.
    pub fn from_file_observed(path: &Path) -> Result<Self, ConversionError> {
        Self::read_file(path, FileObservation::Observed)
    }

    fn read_file(path: &Path, observation: FileObservation) -> Result<Self, ConversionError> {
        let mut file = File::open(path).map_err(|source| {
            if source.kind() == io::ErrorKind::NotFound {
                ConversionError::NoSuchFile {
                    path: path.to_path_buf(),
                }
            } else {
                read_error(path, source)
            }
        })?;
        let metadata = file.metadata().map_err(|source| read_error(path, source))?;
        let bytes = read_capped(&mut file, metadata.len(), MAX_SOUNDBANK_LEN)
            .map_err(|source| read_error(path, source))?;
        let origin = match (observation, &bytes) {
            (FileObservation::Observed, Some(bytes)) => ObservedFileOrigin::of_read(
                path,
                &file,
                FileStamp::from_metadata(&metadata),
                bytes.len(),
            )
            .map_err(|source| read_error(path, source))?
            .map_or(BankReadOrigin::Unobserved, BankReadOrigin::File),
            _ => BankReadOrigin::Unobserved,
        };
        Self::parse(
            path.display().to_string(),
            bytes,
            SourceProtection::new(None),
            origin,
        )
    }

    /// Returns the original Entry of an observed Archive read.
    #[must_use]
    pub const fn archive_origin(&self) -> Option<&EntryOrigin> {
        match &self.origin {
            BankReadOrigin::Archive(origin) => Some(origin),
            BankReadOrigin::Unobserved | BankReadOrigin::File(_) => None,
        }
    }

    pub(crate) const fn observed_input(&self) -> SourceInput<'_> {
        match &self.origin {
            BankReadOrigin::Archive(origin) => SourceInput::Archive(origin),
            BankReadOrigin::File(origin) => SourceInput::File(origin),
            BankReadOrigin::Unobserved => SourceInput::Unknown,
        }
    }
}
