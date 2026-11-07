// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use thiserror::Error;

/// A bounded metadata-access failure retaining its original I/O or budget cause.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MetadataReadError {
    /// The span belongs to a different borrowed source.
    #[error("metadata span belongs to another owning source")]
    ForeignSource,
    /// An Entry is not a physical Entry of this view's Archive.
    #[error("metadata Entry belongs to another Archive")]
    ForeignEntry,
    /// Recovery did not establish a global directory layout.
    #[error("recovered inventory has no proved global metadata layout")]
    UnavailableLayout,
    /// The retained declaration cannot establish this metadata section.
    #[error("metadata section has an invalid retained declaration")]
    MalformedSection,
    /// Declared records leave uninterpreted bytes in their directory section.
    #[error("unparsed directory bytes at owning-source offset {offset} ({len} bytes)")]
    UnparsedDirectoryTail {
        /// Owning-source offset of the uninterpreted bytes.
        offset: u64,
        /// Number of uninterpreted bytes.
        len: u64,
    },
    /// A prior failure prevents further proven iteration.
    #[error("metadata iteration cannot continue after a malformed record")]
    FailedIteration,
    /// The shared layout parser rejected a metadata header.
    #[error("metadata header parse failed at owning-source offset {offset}")]
    Parse {
        /// Owning-source offset of the rejected header.
        offset: u64,
        /// Original parser failure.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// The requested chunk exceeds the metadata-access limit.
    #[error("metadata chunk exceeds {maximum} bytes")]
    ChunkLimit {
        /// Maximum caller buffer length in bytes.
        maximum: usize,
    },
    /// The read does not fit into the supplied metadata span.
    #[error("metadata read exceeds its checked span")]
    OutsideSpan,
    /// The actual cancellation flag is set.
    #[error("metadata access was cancelled")]
    Cancelled,
    /// The shared caller-owned budget refused an operation.
    #[error("metadata budget refused the operation")]
    Budget(#[source] io::Error),
    /// The underlying source read failed.
    #[error("metadata source read failed at owning-source offset {offset}")]
    Read {
        /// Owning-source offset of the requested read.
        offset: u64,
        /// Original source I/O failure.
        #[source]
        source: io::Error,
    },
    /// A directory-record header cannot be interpreted within its declared section.
    #[error("metadata record is malformed at owning-source offset {offset}")]
    MalformedRecord {
        /// Owning-source offset of the malformed record.
        offset: u64,
    },
}
