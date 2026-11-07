// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::ffi::OsStr;
use std::fs::{self, File, Metadata};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::buffers::Buffers;
use crate::convert::{PEEK_LEN, is_cryxml};
use crate::entry_path::{escaped_raw_path, relative_path};
use crate::portable_name::check_path_length;
use crate::source::{EntrySource, SourceEntry, SourceReader};
use crate::tracker::Tracker;
use crate::{
    ArchiveWriteProtection, AudioConversion, CancelToken, ConflictPolicy, ExtractionError,
    TextureConversion,
};

/// How one Entry of an Extraction ended.
pub(crate) enum Outcome {
    Written,
    Converted,
    Kept,
    /// Left out because a run that copies only converted files found nothing to convert.
    NotConverted,
    Cancelled,
    Failed(ExtractionError),
}

pub(crate) enum Chunk {
    Data(usize),
    End,
    Cancelled,
}

/// Where one Entry goes: its folder, created below the target, its file in that folder and the
/// metadata of a file already there.
pub(crate) struct Destination {
    pub(crate) folder: PathBuf,
    pub(crate) file: PathBuf,
    pub(crate) existing: Option<Metadata>,
}

impl Destination {
    /// Returns where the file `name` in the existing `folder` goes, with the metadata of a file
    /// already there.
    pub(crate) fn new(folder: PathBuf, name: &OsStr) -> Result<Self, ExtractionError> {
        let file = folder.join(name);
        check_path_length(&file)?;
        let existing = match fs::symlink_metadata(&file) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(source) => return Err(read_existing_error(&file, source)),
        };
        Ok(Self {
            folder,
            file,
            existing,
        })
    }
}

/// Writes the files of one source below one target folder under one Conflict policy.
pub(crate) struct EntryWriter<'a> {
    pub(crate) protection: Option<&'a ArchiveWriteProtection>,
    pub(crate) cancel: Option<&'a CancelToken>,
    pub(crate) source: &'a dyn EntrySource,
    pub(crate) target: &'a Path,
    pub(crate) policy: ConflictPolicy,
    pub(crate) convert: bool,
    /// Whether a file that Conversion leaves unchanged is written too; without it such a file is
    /// [`Outcome::NotConverted`].
    pub(crate) copy_other: bool,
    pub(crate) textures: TextureConversion,
    pub(crate) audio: AudioConversion,
    pub(crate) verify: bool,
    pub(crate) tracker: &'a Tracker<'a>,
}

impl EntryWriter<'_> {
    pub(crate) fn check_output(&self, target: &Path) -> Result<(), ExtractionError> {
        crate::check_protected_output(target, self.protection, self.cancel)?;
        Ok(())
    }

    pub(crate) fn folder(&self, relative: Option<&Path>) -> Result<PathBuf, ExtractionError> {
        self.tracker
            .folders
            .folder_protected(self.target, relative, self.protection, self.cancel)
    }

    pub(crate) fn temporary(
        &self,
        folder: &Path,
    ) -> Result<(PathBuf, File, crate::OwnedFileToken), ExtractionError> {
        crate::output_creation::create_temporary_protected(folder, self.protection, self.cancel)
    }
    /// Opens the data of `entry`, checked against its directory checksum when the run verifies.
    pub(crate) fn open_entry<'e>(
        &self,
        entry: &SourceEntry<'e>,
    ) -> Result<SourceReader<'e>, ExtractionError> {
        entry.open(self.verify)
    }
}

impl EntryWriter<'_> {
    /// Writes `entry`, using `buffers` for its data, and reports its end to the tracker.
    ///
    /// An Entry that failed because the watched Archive changed counts as cancelled.
    pub(crate) fn write(&self, entry: &SourceEntry<'_>, buffers: &mut Buffers) -> Outcome {
        self.write_entry(entry, buffers, self.convert)
    }

    /// Writes `entry` unchanged, even with Conversion on, and reports its end to the tracker.
    pub(crate) fn write_raw(&self, entry: &SourceEntry<'_>, buffers: &mut Buffers) -> Outcome {
        self.write_entry(entry, buffers, false)
    }

    fn write_entry(
        &self,
        entry: &SourceEntry<'_>,
        buffers: &mut Buffers,
        convert: bool,
    ) -> Outcome {
        let mut written = 0;
        let outcome = match self.try_write(entry, buffers, convert, &mut written) {
            Ok(outcome) => outcome,
            Err(error) if crate::guard_cancellation::retained_output(&error) => {
                Outcome::Failed(error)
            }
            Err(_) if self.tracker.archive_file_changed() => Outcome::Cancelled,
            Err(error) => Outcome::Failed(error),
        };
        buffers.trim();
        self.tracker.ended(entry, written);
        outcome
    }

    fn try_write(
        &self,
        entry: &SourceEntry<'_>,
        buffers: &mut Buffers,
        convert: bool,
        written: &mut u64,
    ) -> Result<Outcome, ExtractionError> {
        if self.tracker.should_stop() {
            return Ok(Outcome::Cancelled);
        }
        let mut prefix = [0; PEEK_LEN];
        let sniffed = if convert && !self.copy_other {
            let Some((reader, count)) = self.open_sniffed(entry, &mut prefix, written)? else {
                return Ok(Outcome::Cancelled);
            };
            if !is_cryxml(prefix.get(..count).unwrap_or_default()) {
                return Ok(Outcome::NotConverted);
            }
            Some((reader, count))
        } else {
            None
        };
        self.write_file(entry, buffers, (convert, sniffed), (&mut prefix, written))
    }

    /// Writes `entry` to its destination: converted when it is `CryXml` and `convert` is set,
    /// otherwise unchanged. `sniffed` is the reader of the Entry and the length of the prefix
    /// already read into `prefix`, when the caller looked at it before.
    fn write_file(
        &self,
        entry: &SourceEntry<'_>,
        buffers: &mut Buffers,
        (convert, mut sniffed): (bool, Option<(SourceReader<'_>, usize)>),
        (prefix, written): (&mut [u8; PEEK_LEN], &mut u64),
    ) -> Result<Outcome, ExtractionError> {
        let destination = self.destination(entry)?;
        if destination.existing.is_some() && self.policy == ConflictPolicy::Skip {
            return Ok(Outcome::Kept);
        }
        if convert && sniffed.is_none() {
            let Some(opened) = self.open_sniffed(entry, prefix, written)? else {
                return Ok(Outcome::Cancelled);
            };
            sniffed = Some(opened);
        }
        let mut peeked = 0;
        let opened = if let Some((mut reader, count)) = sniffed {
            peeked = count;
            let start = prefix.get(..peeked).unwrap_or_default();
            if is_cryxml(start) {
                return self.convert(entry, &mut reader, start, buffers, &destination, written);
            }
            Some(reader)
        } else {
            None
        };
        let Some(checksum) = self.stated_checksum(entry, &destination, &mut buffers.chunk)? else {
            return Ok(Outcome::Cancelled);
        };
        let expected = || (entry.uncompressed_size(), checksum);
        if let Some(outcome) = self.decide(&destination, expected, &mut buffers.chunk)? {
            return Ok(outcome);
        }
        let mut reader = match opened {
            Some(reader) => reader,
            None => self.open_entry(entry)?,
        };
        let (temporary, mut file, ownership) = self.temporary(&destination.folder)?;
        let start = prefix.get(..peeked).unwrap_or_default();
        let copied = file
            .write_all(start)
            .map_err(|source| write_error(destination.file.clone(), source))
            .and_then(|()| {
                self.copy(
                    &mut reader,
                    file,
                    &mut buffers.chunk,
                    &destination.file,
                    written,
                )
            });
        self.put_in_place(
            &temporary,
            &ownership,
            &destination.file,
            copied,
            Outcome::Written,
        )
    }

    /// Opens `entry` and reads its first bytes into `prefix`; returns the reader and how many
    /// bytes were read, or `None` once the run is to stop.
    fn open_sniffed<'e>(
        &self,
        entry: &SourceEntry<'e>,
        prefix: &mut [u8; PEEK_LEN],
        written: &mut u64,
    ) -> Result<Option<(SourceReader<'e>, usize)>, ExtractionError> {
        let mut reader = self.open_entry(entry)?;
        Ok(self
            .peek(&mut reader, prefix, written)?
            .map(|count| (reader, count)))
    }

    /// Returns the path `entry` is named by on disk: its path, or for an Entry whose stored path
    /// is not UTF-8 the stored bytes with invalid ones escaped, which differs between Entries
    /// whose lossy paths are equal.
    pub(crate) fn disk_name<'a>(&self, entry: &'a SourceEntry<'_>) -> Cow<'a, str> {
        self.source.raw_path(entry.index()).map_or_else(
            || Cow::Borrowed(entry.path()),
            |raw| Cow::Owned(escaped_raw_path(raw)),
        )
    }

    /// Returns the path below the target `entry` is written to, or why it is not written.
    pub(crate) fn relative_path(
        &self,
        entry: &SourceEntry<'_>,
    ) -> Result<PathBuf, ExtractionError> {
        relative_path(&self.disk_name(entry))
    }

    /// Creates the folder of `entry` below the target and returns where its file goes; fails when
    /// the path is no valid name, too long for Windows or Linux, or collides ignoring case with a
    /// file an earlier Entry of the run wrote.
    pub(crate) fn destination(
        &self,
        entry: &SourceEntry<'_>,
    ) -> Result<Destination, ExtractionError> {
        let relative = self.relative_path(entry)?;
        self.check_output(&self.target.join(&relative))?;
        check_path_length(&self.target.join(&relative))?;
        self.case_collision(&relative, entry)?;
        let name = relative.file_name().ok_or(ExtractionError::UnsafePath)?;
        let folder = self.folder(relative.parent())?;
        Destination::new(folder, name)
    }

    /// Renames `temporary` to `destination` once `copied` reached the end of the data, or removes
    /// it; returns `done` once the file is in place.
    pub(crate) fn put_in_place(
        &self,
        temporary: &Path,
        ownership: &crate::OwnedFileToken,
        destination: &Path,
        copied: Result<Chunk, ExtractionError>,
        done: Outcome,
    ) -> Result<Outcome, ExtractionError> {
        if !matches!(copied, Ok(Chunk::End)) || self.tracker.archive_file_changed() {
            return crate::output_cleanup::extraction(
                copied.map(|_| Outcome::Cancelled),
                ownership,
                temporary,
                self.cancel,
            );
        }
        let result = (|| {
            ownership.ensure_writable(temporary, crate::output_file::cancel_flag(self.cancel))?;
            self.check_output(destination)?;
            fs::rename(temporary, destination)
                .map_err(|source| write_error(destination.to_path_buf(), source))?;
            Ok(done)
        })();
        if result.is_err() {
            crate::output_cleanup::extraction(result, ownership, temporary, self.cancel)
        } else {
            result
        }
    }

    fn copy(
        &self,
        reader: &mut impl Read,
        mut file: File,
        buffer: &mut [u8],
        destination: &Path,
        written: &mut u64,
    ) -> Result<Chunk, ExtractionError> {
        loop {
            let count = match self
                .read_chunk(reader, buffer)
                .map_err(ExtractionError::read)?
            {
                Chunk::Data(count) => count,
                end => return Ok(end),
            };
            file.write_all(buffer.get(..count).unwrap_or_default())
                .map_err(|source| write_error(destination.to_path_buf(), source))?;
            *written += count as u64;
            self.tracker.wrote(count as u64);
        }
    }

    pub(crate) fn read_chunk(
        &self,
        reader: &mut impl Read,
        buffer: &mut [u8],
    ) -> io::Result<Chunk> {
        loop {
            if self.tracker.should_stop() {
                return Ok(Chunk::Cancelled);
            }
            match reader.read(buffer) {
                Ok(0) => return Ok(Chunk::End),
                Ok(count) => return Ok(Chunk::Data(count)),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
    }
}

pub(crate) const fn write_error(path: PathBuf, source: io::Error) -> ExtractionError {
    ExtractionError::Write { path, source }
}

pub(crate) fn read_existing_error(path: &Path, source: io::Error) -> ExtractionError {
    ExtractionError::ReadExisting {
        path: path.to_path_buf(),
        source,
    }
}
