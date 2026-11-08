// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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
#[derive(Debug)]
#[non_exhaustive]
pub enum MediaError {
    /// Source enumeration is already known to be incomplete before retention starts.
    IncompleteInventory {
        /// Root Archive with global inventory uncertainty, without an invented Entry identity.
        archive: PathBuf,
    },
    /// Normal publication encountered diagnosed projection loss.
    ProjectionLoss {
        /// Provider whose record could not be complete.
        kind: MediaKind,
        /// Original diagnoses of the failed or partial projection.
        issues: Vec<crate::MediaIssue>,
    },
    /// One exact source Entry could not be retained.
    Extraction {
        /// Original typed failure without another decode.
        cause: Box<ExtractionError>,
        /// Counts observed before consuming the extractor report.
        report: ExtractionCounts,
    },
    /// One-Entry Extraction stopped without an Entry failure.
    ExtractionInterrupted {
        /// Original cancellation or Archive-change failure.
        cause: Box<Self>,
        /// Counts actually returned by the interrupted extractor.
        report: ExtractionCounts,
    },
    /// A failure with the exact physical source at which it occurred.
    Source {
        /// Root identity, ordered containers and leaf physical Entry.
        entry: Box<crate::Source>,
        /// Original typed operation failure.
        cause: Box<Self>,
    },
    /// A container could not be enumerated.
    Archive(Box<ArchiveError>),
    /// Source-bound layout access retained its original parser or I/O cause.
    Metadata(Box<p4kx_extraction::MetadataReadError>),
    /// A requested provider is absent or unavailable.
    Unsupported {
        /// Requested kind.
        kind: MediaKind,
        /// Availability explanation.
        detail: String,
    },
    /// Physical candidate inventory could not be indexed completely.
    ReferenceIndex(PathIndexError),
    /// A declared limit was exhausted.
    Limit {
        /// Budget name.
        budget: &'static str,
        /// Configured maximum.
        limit: u64,
    },
    /// The caller cancelled the generation.
    Cancelled,
    /// The source changed during the generation.
    Changed(ArchiveChanged),
    /// Installation protection or source/target identity prevents writing.
    Protected(WriteProtectionError),
    /// Cleanup stopped at an uncertain or inaccessible owned output.
    Cleanup(OwnedOutputCleanupError),
    /// An exclusively created output or its cleanup retained a typed failure.
    OwnedOutput(OwnedOutputError),
    /// A sticky output failure with an independently failed encoder callback when present.
    Output {
        /// Original typed failure shared only between the sink and its I/O result.
        cause: Arc<Self>,
        /// Separate callback failure that does not contain the original sink failure.
        callback: Option<Box<Self>>,
    },
    /// Both an output operation and its post-operation source check failed.
    OutputCheckFailed {
        /// Original typed operation failure.
        operation: Box<Self>,
        /// Original typed cancellation or source-change failure.
        checkpoint: Box<Self>,
    },
    /// Both a required-input operation and its post-operation source check failed.
    InputCheckFailed {
        /// Original typed operation failure.
        operation: Box<Self>,
        /// Original typed post-operation cancellation or source-change failure.
        checkpoint: Box<Self>,
    },
    /// Both the primary operation and its explicit cleanup failed.
    CleanupFailed {
        /// The original operation failure.
        primary: Box<Self>,
        /// The failure that prevented complete cleanup.
        cleanup: Box<Self>,
    },
    /// A successful rename could not authenticate its new output spelling.
    RetainedOutput {
        /// The renamed output kept for recovery.
        path: PathBuf,
        /// The original post-rename failure.
        cause: Box<Self>,
    },
    /// A file operation failed.
    Io {
        /// File or directory being accessed.
        path: PathBuf,
        /// Underlying failure.
        source: io::Error,
    },
    /// The output is not a compatible completed media export.
    Destination(String),
    /// A provider or extracted source violated its contract.
    Contract(String),
    /// Serialisation failed before an artifact could be committed.
    Json(serde_json::Error),
}

impl Display for MediaError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncompleteInventory { archive } => write!(
                f,
                "{} has an incomplete Entry inventory; use diagnostic export to retain an explicitly incomplete generation",
                archive.display()
            ),
            Self::ProjectionLoss { kind, .. } => write!(
                f,
                "media provider {kind} reported projection loss; request diagnostic export to retain it"
            ),
            Self::Extraction { cause, .. } => write!(f, "cannot retain media Entry: {cause}"),
            Self::ExtractionInterrupted { cause, .. } => {
                write!(f, "media Entry retention stopped: {cause}")
            }
            Self::Source { cause, .. } => write!(f, "media source operation failed: {cause}"),
            Self::Archive(cause) => write!(f, "cannot enumerate media container: {cause}"),
            Self::Metadata(cause) => write!(f, "cannot read Archive source metadata: {cause}"),
            Self::Unsupported { kind, detail } => {
                write!(f, "media capability {kind} is unavailable: {detail}")
            }
            Self::ReferenceIndex(cause) => write!(f, "media reference index failed: {cause}"),
            Self::Limit { budget, limit } => {
                write!(f, "media budget {budget} exceeded its limit {limit}")
            }
            Self::Cancelled => f.write_str("media export cancelled"),
            Self::Changed(cause) => Display::fmt(cause, f),
            Self::Protected(cause) => Display::fmt(cause, f),
            Self::Cleanup(cause) => Display::fmt(cause, f),
            Self::OwnedOutput(cause) => Display::fmt(cause, f),
            Self::Output { cause, callback } => write_output(cause, callback.as_deref(), f),
            Self::OutputCheckFailed {
                operation,
                checkpoint,
            } => write!(
                f,
                "output failed: {operation}; source check also failed: {checkpoint}"
            ),
            Self::InputCheckFailed {
                operation,
                checkpoint,
            } => write!(
                f,
                "required input failed: {operation}; source check also failed: {checkpoint}"
            ),
            Self::CleanupFailed { primary, cleanup } => write!(
                f,
                "media operation failed: {primary}; cleanup also failed: {cleanup}"
            ),
            Self::RetainedOutput { path, cause } => {
                write!(f, "media output {} was retained: {cause}", path.display())
            }
            Self::Io { path, source } => write!(f, "media I/O at {}: {source}", path.display()),
            Self::Destination(detail) => write!(f, "invalid media export destination: {detail}"),
            Self::Contract(detail) => write!(f, "invalid media provider result: {detail}"),
            Self::Json(cause) => write!(f, "media JSON error: {cause}"),
        }
    }
}

fn write_output(
    cause: &MediaError,
    callback: Option<&MediaError>,
    f: &mut Formatter<'_>,
) -> fmt::Result {
    write!(f, "converted output failed: {cause}")?;
    if let Some(callback) = callback {
        write!(f, "; encoder also failed: {callback}")?;
    }
    Ok(())
}

impl Error for MediaError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Extraction { cause, .. } => Some(cause.as_ref()),
            Self::ExtractionInterrupted { cause, .. }
            | Self::Source { cause, .. }
            | Self::RetainedOutput { cause, .. } => Some(cause.as_ref()),
            Self::Archive(cause) => Some(cause.as_ref()),
            Self::Metadata(cause) => Some(cause.as_ref()),
            Self::ReferenceIndex(cause) => Some(cause),
            Self::Changed(cause) => cause.source(),
            Self::Protected(cause) => cause.source(),
            Self::Cleanup(cause) => cause.source(),
            Self::OwnedOutput(cause) => cause.source(),
            Self::Output { cause, .. } => Some(cause.as_ref()),
            Self::OutputCheckFailed { operation, .. }
            | Self::InputCheckFailed { operation, .. } => Some(operation.as_ref()),
            Self::CleanupFailed { primary, .. } => Some(primary.as_ref()),
            Self::Io { source, .. } => Some(source),
            Self::Json(cause) => Some(cause),
            Self::IncompleteInventory { .. }
            | Self::ProjectionLoss { .. }
            | Self::Unsupported { .. }
            | Self::Limit { .. }
            | Self::Cancelled
            | Self::Destination(_)
            | Self::Contract(_) => None,
        }
    }
}

impl From<ArchiveChanged> for MediaError {
    fn from(cause: ArchiveChanged) -> Self {
        Self::Changed(cause)
    }
}

impl From<serde_json::Error> for MediaError {
    fn from(cause: serde_json::Error) -> Self {
        Self::Json(cause)
    }
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

#[must_use]
pub(crate) fn input(cause: MediaError) -> MediaError {
    match cause {
        MediaError::Io { path, source } => match source.downcast::<MediaError>() {
            Ok(cause) => input(cause),
            Err(source) => MediaError::Io { path, source },
        },
        MediaError::Source { entry, cause } => MediaError::Source {
            entry,
            cause: Box::new(input(*cause)),
        },
        MediaError::ExtractionInterrupted { cause, report } => MediaError::ExtractionInterrupted {
            cause: Box::new(input(*cause)),
            report,
        },
        cause => cause,
    }
}
