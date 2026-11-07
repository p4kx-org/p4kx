// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};

use super::{Error, ErrorKind};

/// Validates and normalizes a bounded relative reference path.
///
/// # Errors
///
/// Returns an error for absolute, ambiguous, parent or oversized paths.
pub fn relative_path(text: &str, max_bytes: usize) -> Result<String, Error> {
    if text.len() > max_bytes {
        return Err(Error::new(
            ErrorKind::Bounds,
            "relative path exceeds its bound",
        ));
    }
    let path = text.replace('\\', "/");
    if path.is_empty()
        || path.starts_with('/')
        || path.contains(':')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(Error::new(
            ErrorKind::Path,
            "not a bounded relative reference path",
        ));
    }
    Ok(path)
}

/// Returns whether metadata names a symlink or platform reparse point.
#[cfg(windows)]
#[must_use]
pub fn is_linked(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const REPARSE_POINT_ATTRIBUTE: u32 = 0x400;
    metadata.file_attributes() & REPARSE_POINT_ATTRIBUTE != 0
}

/// Returns whether metadata names a symlink or platform reparse point.
#[cfg(not(windows))]
#[must_use]
pub fn is_linked(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink()
}

/// Canonicalizes a direct directory used as a local reference root.
///
/// # Errors
///
/// Returns an error for missing, linked or non-directory roots.
pub fn direct_root(path: &Path) -> Result<PathBuf, Error> {
    let metadata = fs::symlink_metadata(path).map_err(|error| Error::new(ErrorKind::Io, error))?;
    if !metadata.is_dir() || is_linked(&metadata) {
        return Err(Error::new(
            ErrorKind::Path,
            "reference root must be a direct directory",
        ));
    }
    fs::canonicalize(path).map_err(|error| Error::new(ErrorKind::Io, error))
}

/// Resolves a direct regular file within a canonical reference root.
///
/// # Errors
///
/// Returns an error for linked ancestors, escapes or invalid relative paths.
pub fn contained_file(root: &Path, relative: &str, max_bytes: usize) -> Result<PathBuf, Error> {
    let relative = relative_path(relative, max_bytes)?;
    let mut path = root.to_path_buf();
    for component in relative.split('/') {
        path.push(component);
        let metadata =
            fs::symlink_metadata(&path).map_err(|error| Error::new(ErrorKind::Io, error))?;
        if is_linked(&metadata) {
            return Err(Error::new(
                ErrorKind::Path,
                "linked reference path is not accepted",
            ));
        }
    }
    let resolved = fs::canonicalize(path).map_err(|error| Error::new(ErrorKind::Io, error))?;
    if !resolved.starts_with(root) || !resolved.is_file() {
        return Err(Error::new(
            ErrorKind::Path,
            "reference resolves outside root or is not a file",
        ));
    }
    Ok(resolved)
}
