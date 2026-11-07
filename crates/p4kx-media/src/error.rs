// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};

use p4kx_extraction::{
    ArchiveChanged, ArchiveError, ExtractionError, OwnedOutputCleanupError, OwnedOutputError,
    PathIndexError, WriteProtectionError,
};

use crate::MediaKind;

/// Actual one-Entry Extraction counts retained alongside its original error.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct ExtractionCounts {
    /// Physical Entries actually extracted.
    pub extracted: usize,
    /// Physical Entries skipped by the extractor.
    pub skipped: usize,
    /// Physical Entry failures.
    pub failed: usize,
    /// Physical Entries cancelled.
    pub cancelled: usize,
    /// Whether the extractor observed a changed Archive.
    pub archive_changed: bool,
}

/// A failure that prevents publishing a complete generation.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MediaError {
    /// Source enumeration is already known to be incomplete before retention starts.
    #[error("{} has an incomplete Entry inventory; use diagnostic export to retain an explicitly incomplete generation", archive.display())]
    IncompleteInventory {
        /// Root Archive with global inventory uncertainty, without an invented Entry identity.
        archive: PathBuf,
    },
    /// Normal publication encountered diagnosed projection loss.
    #[error(
        "media provider {kind} reported projection loss; request diagnostic export to retain it"
    )]
    ProjectionLoss {
        /// Provider whose record could not be complete.
        kind: MediaKind,
        /// Original diagnoses of the failed or partial projection.
        issues: Vec<crate::MediaIssue>,
    },
    /// One exact source Entry could not be retained.
    #[error("cannot retain media Entry: {cause}")]
    Extraction {
        /// Original typed failure without another decode.
        #[source]
        cause: Box<ExtractionError>,
        /// Counts observed before consuming the extractor report.
        report: ExtractionCounts,
    },
    /// One-Entry Extraction stopped without an Entry failure.
    #[error("media Entry retention stopped: {cause}")]
    ExtractionInterrupted {
        /// Original cancellation or Archive-change failure.
        #[source]
        cause: Box<Self>,
        /// Counts actually returned by the interrupted extractor.
        report: ExtractionCounts,
    },
    /// A failure with the exact physical source at which it occurred.
    #[error("media source operation failed: {cause}")]
    Source {
        /// Root identity, ordered containers and leaf physical Entry.
        entry: Box<crate::Source>,
        /// Original typed operation failure.
        #[source]
        cause: Box<Self>,
    },
    /// A container could not be enumerated.
    #[error("cannot enumerate media container: {0}")]
    Archive(#[source] Box<ArchiveError>),
    /// Source-bound layout access retained its original parser or I/O cause.
    #[error("cannot read Archive source metadata: {0}")]
    Metadata(#[source] Box<p4kx_extraction::MetadataReadError>),
    /// A requested provider is absent or unavailable.
    #[error("media capability {kind} is unavailable: {detail}")]
    Unsupported {
        /// Requested kind.
        kind: MediaKind,
        /// Availability explanation.
        detail: String,
    },
    /// Physical candidate inventory could not be indexed completely.
    #[error("media reference index failed: {0}")]
    ReferenceIndex(#[source] PathIndexError),
    /// A declared limit was exhausted.
    #[error("media budget {budget} exceeded its limit {limit}")]
    Limit {
        /// Budget name.
        budget: &'static str,
        /// Configured maximum.
        limit: u64,
    },
    /// The caller cancelled the generation.
    #[error("media export cancelled")]
    Cancelled,
    /// The source changed during the generation.
    #[error(transparent)]
    Changed(#[from] ArchiveChanged),
    /// Installation protection or source/target identity prevents writing.
    #[error(transparent)]
    Protected(WriteProtectionError),
    /// Cleanup stopped at an uncertain or inaccessible owned output.
    #[error(transparent)]
    Cleanup(OwnedOutputCleanupError),
    /// An exclusively created output or its cleanup retained a typed failure.
    #[error(transparent)]
    OwnedOutput(OwnedOutputError),
    /// Both the primary operation and its explicit cleanup failed.
    #[error("media operation failed: {primary}; cleanup also failed: {cleanup}")]
    CleanupFailed {
        /// The original operation failure.
        #[source]
        primary: Box<Self>,
        /// The failure that prevented complete cleanup.
        cleanup: Box<Self>,
    },
    /// A successful rename could not authenticate its new output spelling.
    #[error("media output {} was retained: {cause}", path.display())]
    RetainedOutput {
        /// The renamed output kept for recovery.
        path: PathBuf,
        /// The original post-rename failure.
        #[source]
        cause: Box<Self>,
    },
    /// A file operation failed.
    #[error("media I/O at {}: {source}", path.display())]
    Io {
        /// File or directory being accessed.
        path: PathBuf,
        /// Underlying failure.
        #[source]
        source: io::Error,
    },
    /// The output is not a compatible completed media export.
    #[error("invalid media export destination: {0}")]
    Destination(String),
    /// A provider or extracted source violated its contract.
    #[error("invalid media provider result: {0}")]
    Contract(String),
    /// Serialisation failed before an artifact could be committed.
    #[error("media JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<WriteProtectionError> for MediaError {
    fn from(cause: WriteProtectionError) -> Self {
        if cause.is_cancelled() {
            Self::Cancelled
        } else {
            Self::Protected(cause)
        }
    }
}

impl From<p4kx_extraction::MetadataReadError> for MediaError {
    fn from(cause: p4kx_extraction::MetadataReadError) -> Self {
        use p4kx_extraction::MetadataReadError;
        match cause {
            MetadataReadError::Cancelled => Self::Cancelled,
            MetadataReadError::Budget(cause) => match cause.downcast::<Self>() {
                Ok(cause) => cause,
                Err(cause) => Self::Metadata(Box::new(MetadataReadError::Budget(cause))),
            },
            cause => Self::Metadata(Box::new(cause)),
        }
    }
}

impl From<OwnedOutputCleanupError> for MediaError {
    fn from(cause: OwnedOutputCleanupError) -> Self {
        Self::Cleanup(cause)
    }
}

impl From<OwnedOutputError> for MediaError {
    fn from(cause: OwnedOutputError) -> Self {
        match cause {
            OwnedOutputError::Protection(cause) => cause.into(),
            OwnedOutputError::Io { path, source } => match source.downcast::<Self>() {
                Ok(cause) => cause,
                Err(source) => Self::Io { path, source },
            },
            cause => Self::OwnedOutput(cause),
        }
    }
}

pub(crate) fn creation(path: &Path, source: io::Error) -> MediaError {
    if p4kx_extraction::owned_output_failure(&source).is_some_and(OwnedOutputError::is_cancelled) {
        MediaError::Cancelled
    } else {
        io(path, source)
    }
}

pub(crate) fn finish<T>(
    result: Result<T, MediaError>,
    cleanup: Result<(), MediaError>,
) -> Result<T, MediaError> {
    match (result, cleanup) {
        (result, Ok(())) => result,
        (Ok(_), Err(cleanup)) => Err(cleanup),
        (Err(primary), Err(cleanup)) => Err(MediaError::CleanupFailed {
            primary: Box::new(primary),
            cleanup: Box::new(cleanup),
        }),
    }
}

pub(crate) fn io(path: &Path, source: io::Error) -> MediaError {
    match source.downcast::<MediaError>() {
        Ok(cause) => cause,
        Err(source) => MediaError::Io {
            path: path.to_path_buf(),
            source,
        },
    }
}

pub(crate) fn at(entry: &crate::Source, cause: MediaError) -> MediaError {
    MediaError::Source {
        entry: Box::new(entry.clone()),
        cause: Box::new(cause),
    }
}
