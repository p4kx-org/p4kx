// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::PathBuf;

use p4kx_gltf::GltfError;
use p4kx_guard::{OwnedOutputCleanupError, OwnedOutputError, WriteProtectionError, WriteRefused};
use thiserror::Error;

/// Why a package could not be written.
///
/// Errors retain their causes; uncertain temporary targets and unrestored earlier packages remain.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PackageError {
    /// Installation protection or source/target identity prevents writing.
    #[error(transparent)]
    Refused(WriteProtectionError),
    /// Cleanup stopped before modifying an uncertain or inaccessible owned output.
    #[error(transparent)]
    Cleanup(OwnedOutputCleanupError),
    /// Exclusive creation or cleanup retained a typed output failure.
    #[error(transparent)]
    OwnedOutput(OwnedOutputError),
    /// Both the package operation and its explicit cleanup failed.
    #[error("package operation failed: {primary}; cleanup also failed: {cleanup}")]
    CleanupFailed {
        /// The original package failure.
        #[source]
        primary: Box<Self>,
        /// The failure that retained temporary output.
        cleanup: Box<Self>,
    },
    /// A successful rename could not authenticate its destination.
    #[error("package output {} was retained: {cause}", path.display())]
    RetainedOutput {
        /// The renamed output that remains.
        path: PathBuf,
        /// The earlier package retained during replacement.
        previous: Option<PathBuf>,
        /// The original post-rename failure.
        #[source]
        cause: Box<Self>,
    },
    /// Protection prevented restoring the earlier package after a failed replacement.
    #[error("cannot restore the earlier package at {} to {}: {cause}", old.display(), path.display())]
    RestoreRefused {
        /// The requested package output.
        path: PathBuf,
        /// The retained earlier package.
        old: PathBuf,
        /// The protection refusal.
        #[source]
        cause: Box<Self>,
        /// The original replacement failure.
        replacement: Box<Self>,
    },
    /// The output exists and is not a package folder of this tool, so it is not replaced.
    #[error(
        "{} exists and is not a package folder written by this tool; choose a new folder",
        path.display()
    )]
    NotAPackage {
        /// The output as given.
        path: PathBuf,
    },
    /// An earlier retained backup occupies the requested replacement name.
    #[error("a retained package backup already exists at {}", path.display())]
    BackupExists {
        /// The backup left for explicit recovery.
        path: PathBuf,
    },
    /// The output names no folder, e.g. it ends in `..`.
    #[error("{} names no folder", path.display())]
    NoFolderName {
        /// The output as given.
        path: PathBuf,
    },
    /// A file or folder cannot be created, written or moved.
    #[error("cannot write {}", path.display())]
    Write {
        /// What was being written.
        path: PathBuf,
        /// What went wrong.
        #[source]
        source: io::Error,
    },
    /// A folder cannot be moved because another program holds it open, e.g. a virus scanner, the
    /// indexer, an Explorer preview or a shell whose working folder it is.
    #[error("{} is in use by another program; close it and try again", path.display())]
    InUse {
        /// The folder that could not be moved.
        path: PathBuf,
        /// The last error of the attempts.
        #[source]
        source: io::Error,
    },
    /// The new package could not be moved into place and the earlier one could not be moved back;
    /// it is at `old`.
    #[error(
        "cannot move the new package to {} and cannot restore the earlier one, which is at {}",
        path.display(),
        old.display()
    )]
    NotRestored {
        /// The output.
        path: PathBuf,
        /// Where the earlier package is now.
        old: PathBuf,
        /// Why it could not be moved back.
        #[source]
        source: io::Error,
    },
    /// A mesh cannot be written as a GLB file.
    #[error("cannot write the mesh {id} of {name}")]
    Mesh {
        /// The mesh's id.
        id: String,
        /// The class name of the first Entity that draws it.
        name: String,
        /// What went wrong.
        #[source]
        source: GltfError,
    },
    /// Original DDS metadata or package field evidence could not be recorded.
    #[error("cannot record package texture metadata")]
    TextureEvidence(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// Original entity classification evidence could not be recorded.
    #[error("cannot record package entity classification")]
    EntityEvidence(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// The caller asked to stop.
    #[error("cancelled")]
    Cancelled,
}

impl From<WriteRefused> for PackageError {
    fn from(refused: WriteRefused) -> Self {
        Self::Refused(refused.into())
    }
}

impl From<WriteProtectionError> for PackageError {
    fn from(cause: WriteProtectionError) -> Self {
        if cause.is_cancelled() {
            Self::Cancelled
        } else {
            Self::Refused(cause)
        }
    }
}

impl From<OwnedOutputCleanupError> for PackageError {
    fn from(cause: OwnedOutputCleanupError) -> Self {
        Self::Cleanup(cause)
    }
}

impl From<OwnedOutputError> for PackageError {
    fn from(cause: OwnedOutputError) -> Self {
        match cause {
            OwnedOutputError::Protection(cause) => cause.into(),
            OwnedOutputError::Io { path, source } => from_owned_io(path, source),
            cause => Self::OwnedOutput(cause),
        }
    }
}

fn from_owned_io(path: PathBuf, source: io::Error) -> PackageError {
    match source.downcast::<PackageError>() {
        Ok(cause) => cause,
        Err(source) => PackageError::write(path, source),
    }
}

pub(crate) fn finish<T>(
    result: Result<T, PackageError>,
    cleanup: Result<(), PackageError>,
) -> Result<T, PackageError> {
    match (result, cleanup) {
        (result, Ok(())) => result,
        (Ok(_), Err(cleanup)) => Err(cleanup),
        (Err(primary), Err(cleanup)) => Err(PackageError::CleanupFailed {
            primary: Box::new(primary),
            cleanup: Box::new(cleanup),
        }),
    }
}

impl PackageError {
    pub(crate) fn write(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Write {
            path: path.into(),
            source,
        }
    }
}
