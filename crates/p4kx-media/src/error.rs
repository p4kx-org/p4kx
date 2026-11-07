// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};

use p4kx_extraction::{ArchiveChanged, ArchiveError, ExtractionError, WriteRefused};

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
    /// A requested provider is absent or unavailable.
    #[error("media capability {kind} is unavailable: {detail}")]
    Unsupported {
        /// Requested kind.
        kind: MediaKind,
        /// Availability explanation.
        detail: String,
    },
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
    /// The output is in a protected installation.
    #[error(transparent)]
    Protected(#[from] WriteRefused),
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

pub(crate) fn io(path: &Path, source: io::Error) -> MediaError {
    MediaError::Io {
        path: path.to_path_buf(),
        source,
    }
}

pub(crate) fn at(entry: &crate::Source, cause: MediaError) -> MediaError {
    MediaError::Source {
        entry: Box::new(entry.clone()),
        cause: Box::new(cause),
    }
}
