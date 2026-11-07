// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use p4kx_dataforge::{Annotations, DataForgeError, Guid, TagResolver, TextResolver};
use thiserror::Error;

use crate::{CancelToken, ConflictPolicy, ExtractionError, NameRule};

/// The settings of one Record export.
#[derive(Clone, Copy, Default)]
pub struct RecordExportOptions<'a> {
    /// Source-origin protection of an Archive-backed export job.
    pub protection: Option<&'a crate::ArchiveWriteProtection>,
    /// What to do with files that already exist.
    pub policy: ConflictPolicy,
    /// Stops the run once cancelled.
    pub cancel: Option<&'a CancelToken>,
    /// Called from any worker thread once for every Record the run built, kept or gave up on;
    /// calls may overtake each other.
    pub progress: Option<&'a (dyn Fn() + Sync)>,
    /// Resolves the localisation keys of the Records into one language; without it the output
    /// is language-free.
    pub texts: Option<&'a dyn TextResolver>,
    /// Resolves the tag References of the Records into their tag paths, e.g. a
    /// [`TagTree`](p4kx_dataforge::TagTree); without it tag References carry no path.
    pub tags: Option<&'a dyn TagResolver>,
}

impl<'a> RecordExportOptions<'a> {
    /// Returns the annotations the texts and tags of the options add to the Records.
    #[must_use]
    pub const fn annotations(&self) -> Annotations<'a> {
        Annotations::none()
            .with_texts(self.texts)
            .with_tags(self.tags)
    }
}

impl fmt::Debug for RecordExportOptions<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecordExportOptions")
            .field("protection", &self.protection)
            .field("policy", &self.policy)
            .field("cancel", &self.cancel)
            .field("progress", &self.progress.is_some())
            .field("texts", &self.texts.is_some())
            .field("tags", &self.tags.is_some())
            .finish()
    }
}

/// Why one Record of a Record export was not written.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RecordExportError {
    /// The Record tree cannot be built.
    #[error(transparent)]
    Tree(#[from] DataForgeError),
    /// An earlier Record of the run is written to the same file.
    #[error("an earlier Record of this export is written to {path}")]
    FileTaken {
        /// The file, relative to the target.
        path: String,
    },
    /// The file or a folder for it cannot be written, or its record path would leave the target.
    #[error(transparent)]
    Write(#[from] ExtractionError),
    /// The Record belongs to another DataForge than the one exported from.
    #[error("the Record belongs to another DataForge")]
    OtherDataForge,
}

/// One Record that a Record export did not write.
#[derive(Debug)]
pub struct RecordFailure {
    pub(crate) guid: Guid,
    pub(crate) record_path: String,
    pub(crate) error: RecordExportError,
}

impl RecordFailure {
    /// Creates the failure of the Record with `guid` at `record_path`.
    #[must_use]
    pub const fn new(guid: Guid, record_path: String, error: RecordExportError) -> Self {
        Self {
            guid,
            record_path,
            error,
        }
    }

    /// Returns the GUID of the Record.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }

    /// Returns the record path, or an empty text when it cannot be read.
    #[must_use]
    pub fn record_path(&self) -> &str {
        &self.record_path
    }

    /// Returns why the Record was not written.
    #[must_use]
    pub const fn error(&self) -> &RecordExportError {
        &self.error
    }
}

/// Why the file of a Record does not follow its name or record path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum RenameCause {
    /// The Record name is no valid file name, so the GUID names the file.
    #[error("the Record name is no valid file name ({0}); the GUID names the file")]
    Name(NameRule),
    /// A part of the record path is no valid file name, so it was escaped or, when no escape
    /// fits, the Record was put into the folder `%long`.
    #[error("the record path is no valid file name ({0}); the file name is escaped")]
    Path(NameRule),
}

/// One Record whose file does not follow its name or record path.
#[derive(Debug, Clone)]
pub struct RecordRenamed {
    guid: Guid,
    record_path: String,
    cause: RenameCause,
}

impl RecordRenamed {
    /// Creates the note for the Record with `guid` at `record_path`.
    #[must_use]
    pub const fn new(guid: Guid, record_path: String, cause: RenameCause) -> Self {
        Self {
            guid,
            record_path,
            cause,
        }
    }

    /// Returns the GUID of the Record.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }

    /// Returns the record path as the DataForge states it.
    #[must_use]
    pub fn record_path(&self) -> &str {
        &self.record_path
    }

    /// Returns why the file differs.
    #[must_use]
    pub const fn cause(&self) -> RenameCause {
        self.cause
    }
}

/// The outcome of a Record export.
#[derive(Debug, Default)]
pub struct RecordExportReport {
    pub(crate) written: usize,
    pub(crate) skipped: Vec<String>,
    pub(crate) failures: Vec<RecordFailure>,
    pub(crate) renamed: Vec<RecordRenamed>,
    pub(crate) cancelled: usize,
    pub(crate) error_values: usize,
    pub(crate) records_with_errors: usize,
}

impl RecordExportReport {
    /// Returns the number of Records written.
    #[must_use]
    pub const fn written(&self) -> usize {
        self.written
    }

    /// Returns the files, relative to the target, whose existing file was kept.
    #[must_use]
    pub fn skipped(&self) -> &[String] {
        &self.skipped
    }

    /// Returns the Records that failed, with the reason.
    #[must_use]
    pub fn failures(&self) -> &[RecordFailure] {
        &self.failures
    }

    /// Returns the selected Records whose file does not follow their name or record path because
    /// it is no valid file name on both Linux and Windows.
    #[must_use]
    pub fn renamed(&self) -> &[RecordRenamed] {
        &self.renamed
    }

    /// Returns the number of Records not written because the run was cancelled.
    #[must_use]
    pub const fn cancelled(&self) -> usize {
        self.cancelled
    }

    /// Returns the number of error values in the Records written or kept.
    #[must_use]
    pub const fn error_values(&self) -> usize {
        self.error_values
    }

    /// Returns the number of Records written or kept that hold at least one error value.
    #[must_use]
    pub const fn records_with_errors(&self) -> usize {
        self.records_with_errors
    }

    /// Returns the Records that failed, consuming the report.
    #[must_use]
    pub fn into_failures(self) -> Vec<RecordFailure> {
        self.failures
    }

    /// Returns whether no Record failed and the run was not cancelled.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.failures.is_empty() && self.cancelled == 0
    }
}
