// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};

use crate::WriteProtectionError;

/// A protected output creation or filesystem operation could not complete.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum OwnedOutputError {
    /// Source or target evidence refused the operation.
    #[error(transparent)]
    Protection(#[from] WriteProtectionError),
    /// A filesystem operation failed with its original cause.
    #[error("cannot write owned output {}", path.display())]
    Io {
        /// The affected output path.
        path: PathBuf,
        /// The original filesystem failure.
        #[source]
        source: io::Error,
    },
    /// Exclusive creation succeeded but descriptor identity could not be established.
    #[error("new output {} was retained because its identity could not be established", path.display())]
    RetainedCreation {
        /// The newly created output that was not blindly removed.
        path: PathBuf,
        /// The original identity-capture failure.
        #[source]
        source: io::Error,
    },
    /// The primary operation failed and its owned temporary output could not be removed.
    #[error("owned output failed; cleanup also failed: {cleanup}")]
    Cleanup {
        /// The original operation failure.
        #[source]
        primary: Box<Self>,
        /// The secondary cleanup failure.
        cleanup: crate::OwnedOutputCleanupError,
    },
}

impl OwnedOutputError {
    pub(crate) fn io(path: &Path, source: io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            source,
        }
    }

    /// Returns whether cancellation is the only failure of the protected operation.
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        match self {
            Self::Protection(cause) => cause.is_cancelled(),
            Self::Io { .. } | Self::RetainedCreation { .. } | Self::Cleanup { .. } => false,
        }
    }

    /// Preserves the typed failure and filesystem error kind for file factories.
    #[must_use]
    pub fn into_io_error(self) -> io::Error {
        let kind = match &self {
            Self::Protection(_) => io::ErrorKind::PermissionDenied,
            Self::Io { source, .. } => source.kind(),
            Self::RetainedCreation { .. } | Self::Cleanup { .. } => io::ErrorKind::Other,
        };
        io::Error::new(kind, self)
    }
}
