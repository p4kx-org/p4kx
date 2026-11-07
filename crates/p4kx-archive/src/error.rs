// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::PathBuf;

use p4kx_foundation::ParseError;

use crate::Compression;
use thiserror::Error;

/// A failure to open or read an Archive.
#[derive(Debug, Error)]
pub enum ArchiveError {
    /// The directory explicitly declares that installation is incomplete.
    #[error(
        "{path}: installation is incomplete (status {flags:#04x}); wait for the launcher to finish installing"
    )]
    IncompleteInstallation {
        /// Path of the Archive.
        path: PathBuf,
        /// The unmodified source installation-status byte.
        flags: u8,
    },
    /// The Archive file could not be read.
    #[error("cannot read {path}")]
    Io {
        /// Path of the Archive.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// The file has no end-of-central-directory record, so it is not an Archive.
    #[error("{path} is not a p4k Archive: no end of central directory found")]
    NotAnArchive {
        /// Path of the file.
        path: PathBuf,
    },
    /// The Archive is written in a layout this version cannot read.
    #[error("{path} uses Archive layout v{version}, which is not supported")]
    UnsupportedLayout {
        /// Path of the Archive.
        path: PathBuf,
        /// The layout version from the Archive's trailer.
        version: u16,
    },
    /// A structure of the Archive is malformed.
    #[error("{path} is corrupt at offset {offset}: {reason}")]
    Corrupt {
        /// Path of the Archive.
        path: PathBuf,
        /// File offset of the malformed structure.
        offset: u64,
        /// What is wrong.
        reason: String,
    },
    /// A structure of the Archive ended early.
    #[error("{path} is truncated or corrupt")]
    Truncated {
        /// Path of the Archive.
        path: PathBuf,
        /// Where the data ran out.
        #[source]
        source: ParseError,
    },
    /// The offset lies behind the end of the Entry data.
    #[error("{path}: offset {offset} lies beyond the end of Entry {entry} at {size}")]
    OffsetBeyondEnd {
        /// Path of the Archive.
        path: PathBuf,
        /// Path of the Entry inside the Archive.
        entry: String,
        /// The requested offset.
        offset: u64,
        /// The Entry size.
        size: u64,
    },
    /// Reaching the offset in a compressed Entry would decode too much data.
    #[error(
        "{path}: Entry {entry} is compressed; reaching offset {offset} would decode more than {limit} bytes"
    )]
    SkipTooLong {
        /// Path of the Archive.
        path: PathBuf,
        /// Path of the Entry inside the Archive.
        entry: String,
        /// The requested offset.
        offset: u64,
        /// [`MAX_DECODED_SKIP`](crate::MAX_DECODED_SKIP).
        limit: u64,
    },
    /// A container exceeds a limit that guards against nesting, size and compression bombs.
    #[error("{path} exceeds a container limit: {reason}")]
    LimitExceeded {
        /// Location of the container.
        path: PathBuf,
        /// Which limit and by how much.
        reason: String,
    },
    /// An Entry cannot be opened as a container.
    #[error("{path}: Entry {entry} cannot be opened as a container")]
    Container {
        /// Location of the Archive or container holding the Entry.
        path: PathBuf,
        /// Path of the Entry.
        entry: String,
        /// Why the container cannot be opened.
        #[source]
        source: Box<Self>,
    },
    /// The Entry is encrypted and the Archive has no key set.
    #[error("{path}: Entry {entry} is encrypted and no Archive key is set")]
    KeyRequired {
        /// Location of the Archive or container holding the Entry.
        path: PathBuf,
        /// Path of the Entry inside the Archive.
        entry: String,
    },
    /// The Archive key does not decrypt the Entry's data to data matching its checksum.
    #[error("{path}: the Archive key does not decrypt Entry {entry}")]
    WrongKey {
        /// Location of the Archive or container holding the Entry.
        path: PathBuf,
        /// Path of the encrypted Entry the key was tried on.
        entry: String,
    },
    /// The Entry uses a compression method this version cannot read yet.
    #[error("{path}: Entry {entry} uses {compression} compression, which is not supported yet")]
    UnsupportedCompression {
        /// Path of the Archive.
        path: PathBuf,
        /// Path of the Entry inside the Archive.
        entry: String,
        /// The compression method of the Entry.
        compression: Compression,
    },
}

impl ArchiveError {
    /// Returns whether the error, or the container error it wraps, is [`ArchiveError::KeyRequired`].
    #[must_use]
    pub fn is_key_required(&self) -> bool {
        match self {
            Self::KeyRequired { .. } => true,
            Self::Container { source, .. } => source.is_key_required(),
            _ => false,
        }
    }

    /// Returns whether the error, or the container error it wraps, is [`ArchiveError::WrongKey`].
    #[must_use]
    pub fn is_wrong_key(&self) -> bool {
        match self {
            Self::WrongKey { .. } => true,
            Self::Container { source, .. } => source.is_wrong_key(),
            _ => false,
        }
    }
}
