// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use p4kx_foundation::ParseError;

use crate::{ArchiveError, Compression};

/// A failure while reading Archive structures, before the Archive path is attached.
#[derive(Debug)]
pub(crate) enum FormatError {
    Io(io::Error),
    NotAnArchive,
    IncompleteInstallation {
        flags: u8,
    },
    UnsupportedLayout {
        version: u16,
    },
    Corrupt {
        offset: u64,
        reason: String,
    },
    Truncated(ParseError),
    LimitExceeded(String),
    UnsupportedCompression {
        entry: String,
        compression: Compression,
    },
    KeyRequired {
        entry: String,
    },
}

impl FormatError {
    /// Creates a [`FormatError::Corrupt`] for the structure at `offset`.
    pub(crate) fn corrupt(offset: u64, reason: impl Into<String>) -> Self {
        Self::Corrupt {
            offset,
            reason: reason.into(),
        }
    }

    /// Attaches the Archive path.
    pub(crate) fn at(self, path: &Path) -> ArchiveError {
        let path = path.to_path_buf();
        match self {
            Self::Io(source) => ArchiveError::Io { path, source },
            Self::NotAnArchive => ArchiveError::NotAnArchive { path },
            Self::IncompleteInstallation { flags } => {
                ArchiveError::IncompleteInstallation { path, flags }
            }
            Self::UnsupportedLayout { version } => {
                ArchiveError::UnsupportedLayout { path, version }
            }
            Self::Corrupt { offset, reason } => ArchiveError::Corrupt {
                path,
                offset,
                reason,
            },
            Self::Truncated(source) => ArchiveError::Truncated { path, source },
            Self::LimitExceeded(reason) => ArchiveError::LimitExceeded { path, reason },
            Self::UnsupportedCompression { entry, compression } => {
                ArchiveError::UnsupportedCompression {
                    path,
                    entry,
                    compression,
                }
            }
            Self::KeyRequired { entry } => ArchiveError::KeyRequired { path, entry },
        }
    }
}

impl From<io::Error> for FormatError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ParseError> for FormatError {
    fn from(error: ParseError) -> Self {
        Self::Truncated(error)
    }
}
