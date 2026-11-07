// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, Metadata, ReadDir};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use crate::{ArchiveWriteProtection, WriteProtectionError};

/// Why removing an explicitly owned temporary output stopped.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum OwnedOutputCleanupError {
    /// Source or target protection refused a concrete removal.
    #[error(transparent)]
    Protection(#[from] WriteProtectionError),
    /// A filesystem operation failed.
    #[error("cannot clean owned output {}: {source}", path.display())]
    Io {
        /// The affected owned output.
        path: PathBuf,
        /// The original filesystem cause.
        #[source]
        source: io::Error,
    },
    /// An owned output has an unexpected or changed filesystem type.
    #[error("owned output has an unexpected filesystem type: {}", path.display())]
    UnexpectedType {
        /// The affected owned output.
        path: PathBuf,
    },
}

impl OwnedOutputCleanupError {
    /// Returns whether cancellation prevented target authentication during cleanup.
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        match self {
            Self::Protection(cause) => cause.is_cancelled(),
            Self::Io { .. } | Self::UnexpectedType { .. } => false,
        }
    }
}

/// Removes an owned output tree using its Archive job's identity and cancellation evidence.
/// Cancellation does not skip cleanup of ordinary authenticated paths; links are never followed.
///
/// # Errors
/// Returns the first protection, filesystem or unsupported-node failure and retains remaining data.
pub fn remove_owned_output(
    root: &Path,
    protection: &ArchiveWriteProtection,
    cancel: &AtomicBool,
) -> Result<(), OwnedOutputCleanupError> {
    cleanup_checked(root, &|path| {
        protection.ensure_cleanup_writable(path, cancel)
    })
}

/// Removes an owned standalone output with target protection and without following links.
///
/// # Errors
/// Returns the first protection, filesystem or unsupported-node failure and retains remaining data.
pub fn remove_owned_output_target_only(root: &Path) -> Result<(), OwnedOutputCleanupError> {
    cleanup_checked(root, &crate::ensure_writable)
}

struct Directory {
    path: PathBuf,
    entries: ReadDir,
}

impl Directory {
    fn open(path: PathBuf) -> Result<Self, OwnedOutputCleanupError> {
        let entries = fs::read_dir(&path).map_err(|source| io_error(&path, source))?;
        Ok(Self { path, entries })
    }

    fn next(&mut self) -> Result<Option<PathBuf>, OwnedOutputCleanupError> {
        self.entries
            .next()
            .transpose()
            .map(|entry| entry.map(|entry| entry.path()))
            .map_err(|source| io_error(&self.path, source))
    }
}

pub(crate) fn cleanup_checked(
    root: &Path,
    check: &impl Fn(&Path) -> Result<(), WriteProtectionError>,
) -> Result<(), OwnedOutputCleanupError> {
    boundaries(root, check)?;
    let metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(io_error(root, source)),
    };
    if !metadata.is_dir() || redirected(&metadata) {
        remove_leaf(root, &metadata, check)?;
    } else {
        let mut directories = vec![Directory::open(root.to_path_buf())?];
        while let Some(directory) = directories.last_mut() {
            if let Some(child) = directory.next()? {
                concrete(&child, check)?;
                let metadata =
                    fs::symlink_metadata(&child).map_err(|source| io_error(&child, source))?;
                if metadata.is_dir() && !redirected(&metadata) {
                    directories.push(Directory::open(child)?);
                } else {
                    remove_leaf(&child, &metadata, check)?;
                }
            } else {
                let path = directory.path.clone();
                directories.pop();
                remove_directory(&path, check)?;
            }
        }
    }
    boundaries(root, check)
}

fn boundaries(
    root: &Path,
    check: &impl Fn(&Path) -> Result<(), WriteProtectionError>,
) -> Result<(), OwnedOutputCleanupError> {
    for path in root.ancestors() {
        check(path)?;
    }
    Ok(())
}

fn concrete(
    path: &Path,
    check: &impl Fn(&Path) -> Result<(), WriteProtectionError>,
) -> Result<(), OwnedOutputCleanupError> {
    if let Some(parent) = path.parent() {
        check(parent)?;
    }
    check(path)?;
    Ok(())
}

fn remove_directory(
    path: &Path,
    check: &impl Fn(&Path) -> Result<(), WriteProtectionError>,
) -> Result<(), OwnedOutputCleanupError> {
    concrete(path, check)?;
    let metadata = fs::symlink_metadata(path).map_err(|source| io_error(path, source))?;
    if !metadata.is_dir() || redirected(&metadata) {
        return Err(OwnedOutputCleanupError::UnexpectedType { path: path.into() });
    }
    fs::remove_dir(path).map_err(|source| io_error(path, source))
}

fn remove_leaf(
    path: &Path,
    metadata: &Metadata,
    check: &impl Fn(&Path) -> Result<(), WriteProtectionError>,
) -> Result<(), OwnedOutputCleanupError> {
    concrete(path, check)?;
    if !metadata.is_file() && !metadata.file_type().is_symlink() && !redirected(metadata) {
        return Err(OwnedOutputCleanupError::UnexpectedType { path: path.into() });
    }
    let result = if directory_link(metadata) {
        fs::remove_dir(path)
    } else {
        fs::remove_file(path)
    };
    result.map_err(|source| io_error(path, source))
}

pub(crate) fn redirected(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink() || directory_link(metadata)
}

#[cfg(windows)]
fn directory_link(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    let flags = FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT;
    metadata.file_attributes() & flags == flags
}

#[cfg(not(windows))]
const fn directory_link(_metadata: &Metadata) -> bool {
    false
}

fn io_error(path: &Path, source: io::Error) -> OwnedOutputCleanupError {
    OwnedOutputCleanupError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests;
