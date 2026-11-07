// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::PathBuf;

use p4kx_archive::{ArchiveError, Checksum, DataError};
use p4kx_cryxml::CryXmlError;
use p4kx_dataforge::DataForgeError;
use thiserror::Error;

use crate::{ConversionError, NameRule, PathLimit, RecordExportReport};

/// Why one Entry of an Extraction was not written.
#[derive(Debug, Error)]
pub enum ExtractionError {
    /// Ownership capture or verified cleanup failed without losing its typed cause.
    #[error(transparent)]
    OwnedOutput(Box<p4kx_guard::OwnedOutputError>),
    /// An unpublished output could not be removed after completion or failure.
    #[error("owned output cleanup failed: {cleanup}")]
    CleanupFailed {
        /// The original failure when one preceded cleanup.
        #[source]
        primary: Option<Box<dyn std::error::Error + Send + Sync>>,
        /// The separate cleanup failure.
        cleanup: Box<p4kx_guard::OwnedOutputCleanupError>,
    },
    /// The path would leave the target folder, or a folder on the way is a link.
    #[error("the path would leave the target folder")]
    UnsafePath,
    /// A part of the Entry path is not a file name valid on both Linux and Windows; an empty or
    /// dot part, which could leave the target, is one.
    #[error("the name {part:?} cannot be written: {rule}")]
    InvalidName {
        /// The part of the path that breaks the rule.
        part: String,
        /// The rule it breaks.
        rule: NameRule,
    },
    /// The path of the file or its folder is longer than Windows accepts, and so Linux.
    #[error(transparent)]
    PathTooLong(#[from] PathLimit),
    /// A file an earlier Entry of the run wrote differs from this path only in case, or is a file
    /// where this path needs a folder or a folder where it is a file; a case-insensitive file
    /// system cannot hold them side by side.
    #[error("the path collides ignoring case with the earlier Entry {earlier}")]
    CaseCollision {
        /// The path of the earlier Entry.
        earlier: String,
    },
    /// The Entry could not be read from the Archive.
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    /// The Entry data could not be read.
    #[error("cannot read the Entry data")]
    Read(#[source] io::Error),
    /// The file or a folder for the Entry could not be written.
    #[error("cannot write {path}")]
    Write {
        /// The file or folder on disk.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// Source or target protection refused the write.
    #[error(transparent)]
    Installation(#[from] p4kx_guard::WriteProtectionError),
    /// The Entry is `CryXml` that could not be converted.
    #[error("cannot convert the CryXml")]
    Convert(#[source] CryXmlError),
    /// The Entry is `CryXml` or DataForge larger than a Conversion reads into memory.
    #[error("cannot convert the Entry: {size} bytes are more than the limit of {limit}")]
    TooLargeToConvert {
        /// The size of the Entry.
        size: u64,
        /// The largest size converted.
        limit: u64,
    },
    /// The texture could not be converted.
    #[error("cannot convert the texture")]
    Texture(#[source] ConversionError),
    /// The sound, or a WEM the soundbank embeds, could not be converted, naming its codec when no
    /// decoder exists, or the soundbank could not be read.
    #[error("cannot convert the sound")]
    Sound(#[source] Box<ConversionError>),
    /// The Entry is DataForge that could not be opened to convert its Records.
    #[error("cannot convert the DataForge")]
    DataForge(#[source] DataForgeError),
    /// The file already in the target could not be read to compare it with the Entry.
    #[error("cannot read the existing file {path}")]
    ReadExisting {
        /// The existing file.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// The data of the Entry does not match the checksum of the directory; reported only by a
    /// run that verifies, and the Entry is not written.
    #[error("{actual} computed, {expected} stored")]
    ChecksumMismatch {
        /// The checksum the directory states.
        expected: Checksum,
        /// The checksum of the decoded data.
        actual: Checksum,
    },
    /// The worker threads of a run limited by [`ExtractOptions::threads`](crate::ExtractOptions)
    /// could not be started; nothing is written.
    #[error("cannot start the worker threads: {0}")]
    ThreadPool(String),
    /// The file of an input folder cannot be read as an Entry.
    #[error(transparent)]
    Folder(#[from] crate::FolderFileError),
}

impl ExtractionError {
    /// Returns whether the error is about the path of the Entry, which every file of it shares.
    pub(crate) const fn is_path_error(&self) -> bool {
        matches!(
            self,
            Self::UnsafePath
                | Self::InvalidName { .. }
                | Self::PathTooLong(_)
                | Self::CaseCollision { .. }
        )
    }

    /// Returns the error for a failed read of Entry data: a checksum mismatch when that is what
    /// the reader reported, otherwise [`Read`](Self::Read).
    pub(crate) fn read(source: io::Error) -> Self {
        let mismatch = source
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<DataError>());
        match mismatch {
            Some(&DataError::ChecksumMismatch {
                expected, actual, ..
            }) => Self::ChecksumMismatch { expected, actual },
            _ => Self::Read(source),
        }
    }
}

/// One Entry that an Extraction did not write.
#[derive(Debug)]
pub struct EntryFailure {
    pub(crate) path: String,
    pub(crate) error: ExtractionError,
}

impl EntryFailure {
    /// Returns the path of the Entry inside the Archive.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns why the Entry was not written.
    #[must_use]
    pub const fn error(&self) -> &ExtractionError {
        &self.error
    }

    /// Returns the original typed failure without copying or decoding the source again.
    #[must_use]
    pub fn into_error(self) -> ExtractionError {
        self.error
    }

    /// Returns the stored and the computed checksum when the Entry failed because its data does
    /// not match the checksum of the directory.
    #[must_use]
    pub const fn checksum_mismatch(&self) -> Option<(Checksum, Checksum)> {
        match self.error {
            ExtractionError::ChecksumMismatch { expected, actual } => Some((expected, actual)),
            _ => None,
        }
    }
}

/// The outcome of an Extraction.
#[derive(Debug, Default)]
pub struct ExtractionReport {
    pub(crate) extracted: usize,
    pub(crate) converted: Option<usize>,
    pub(crate) skipped: Vec<String>,
    pub(crate) failures: Vec<EntryFailure>,
    pub(crate) unconverted: Vec<EntryFailure>,
    pub(crate) cancelled: Vec<String>,
    pub(crate) not_converted: usize,
    pub(crate) archive_changed: bool,
    pub(crate) records: Option<RecordExportReport>,
    pub(crate) records_entry: Option<String>,
    pub(crate) dataforge_left_out: Vec<String>,
}

impl ExtractionReport {
    /// Returns the number of Entries written.
    #[must_use]
    pub const fn extracted(&self) -> usize {
        self.extracted
    }

    /// Returns the number of Entries written converted, `CryXml` as XML, the Entries of a
    /// converted texture, sounds, soundbanks and the DataForge Entry as Records, or `None` when
    /// the run did not convert; they count among the [`extracted`](Self::extracted) Entries.
    #[must_use]
    pub const fn converted(&self) -> Option<usize> {
        self.converted
    }

    /// Returns the paths of Entries whose existing file in the target was kept.
    #[must_use]
    pub fn skipped(&self) -> &[String] {
        &self.skipped
    }

    /// Returns the Entries that failed, with the reason.
    #[must_use]
    pub fn failures(&self) -> &[EntryFailure] {
        &self.failures
    }

    /// Returns the owned Entry failures, preserving their original typed causes.
    #[must_use]
    pub fn into_failures(self) -> Vec<EntryFailure> {
        self.failures
    }

    /// Returns the Entries among the [`failures`](Self::failures) whose data did not match the
    /// checksum of the directory.
    pub fn checksum_mismatches(&self) -> impl Iterator<Item = &EntryFailure> {
        self.failures
            .iter()
            .filter(|failure| failure.checksum_mismatch().is_some())
    }

    /// Returns the textures, by the path of their header Entry, that a Conversion could not
    /// fully convert, with the reason; their raw Entries were written instead, except a header
    /// whose path the merged DDS already took. Also returns the sounds and soundbanks that could
    /// not be converted, once for each WEM of a soundbank, which were written unchanged.
    #[must_use]
    pub fn unconverted(&self) -> &[EntryFailure] {
        &self.unconverted
    }

    /// Returns the number of files a run that copies only converted files left out because
    /// Conversion has nothing to do with them.
    #[must_use]
    pub const fn not_converted(&self) -> usize {
        self.not_converted
    }

    /// Returns the paths of Entries not written because the run was cancelled.
    #[must_use]
    pub fn cancelled(&self) -> &[String] {
        &self.cancelled
    }

    /// Returns whether the Archive changed on disk before or during the run, which then stopped.
    #[must_use]
    pub const fn archive_changed(&self) -> bool {
        self.archive_changed
    }

    /// Returns the paths of the DataForge files of a folder that a Conversion did not convert
    /// because it converted another one; they are counted in [`not_converted`](Self::not_converted)
    /// or copied.
    #[must_use]
    pub fn dataforge_left_out(&self) -> &[String] {
        &self.dataforge_left_out
    }

    /// Returns the path of the DataForge Entry whose Records a Conversion wrote, or `None` when
    /// none was converted.
    #[must_use]
    pub fn records_entry(&self) -> Option<&str> {
        self.records_entry.as_deref()
    }

    /// Returns the outcome of the Records a Conversion wrote for the DataForge Entry, or `None`
    /// when no DataForge Entry was converted.
    #[must_use]
    pub const fn records(&self) -> Option<&RecordExportReport> {
        self.records.as_ref()
    }

    /// Returns whether no Entry or Record failed, every texture was converted, the run was not
    /// cancelled and the Archive did not change.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        let records = match &self.records {
            Some(records) => records.is_complete(),
            None => true,
        };
        records
            && self.failures.is_empty()
            && self.unconverted.is_empty()
            && self.cancelled.is_empty()
            && !self.archive_changed
    }
}
