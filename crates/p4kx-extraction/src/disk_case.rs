// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Resolves unique ASCII-case variants after exact-name absence (ADR-0332).

use std::collections::HashMap;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, File};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

type Names = HashMap<String, Vec<OsString>>;

#[derive(Debug)]
enum Listing {
    Names(Names),
    Absent,
    Failed(CachedIoError),
}

/// Caches each fallback folder listing, including its failure.
#[derive(Debug, Default)]
pub(crate) struct CaseIndex {
    folders: HashMap<PathBuf, Listing>,
}

impl CaseIndex {
    /// Returns the unique ASCII-case variant, keeping every exact component without backtracking.
    ///
    /// Windows uses exact names only; absent or ambiguous components return `None`.
    ///
    /// # Errors
    ///
    /// Returns filesystem failures other than exact-name absence.
    pub(crate) fn variant(&mut self, path: &Path) -> io::Result<Option<PathBuf>> {
        let mut found = PathBuf::new();
        for component in path.components() {
            let Component::Normal(name) = component else {
                found.push(component);
                continue;
            };
            let exact = found.join(name);
            match fs::symlink_metadata(&exact) {
                Ok(_) => found = exact,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    if cfg!(windows) {
                        return Ok(None);
                    }
                    let Some(actual) = self.unique_match(&found, name)? else {
                        return Ok(None);
                    };
                    found.push(actual);
                }
                Err(error) => return Err(error),
            }
        }
        Ok(Some(found))
    }

    /// Opens a unique variant and returns its actual path, or `None` for absence or ambiguity.
    ///
    /// # Errors
    ///
    /// Returns filesystem failures other than exact-name absence.
    pub(crate) fn open_variant(&mut self, path: &Path) -> io::Result<Option<(File, PathBuf)>> {
        let Some(found) = self.variant(path)? else {
            return Ok(None);
        };
        match File::open(&found) {
            Ok(file) => Ok(Some((file, found))),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn unique_match(&mut self, folder: &Path, name: &OsStr) -> io::Result<Option<OsString>> {
        let listing = self
            .folders
            .entry(folder.to_path_buf())
            .or_insert_with(|| list(folder));
        let names = match listing {
            Listing::Names(names) => names,
            Listing::Absent => return Ok(None),
            Listing::Failed(error) => return Err(error.replay()),
        };
        let Some(names) = names.get(&key(name)) else {
            return Ok(None);
        };
        let mut candidates = names
            .iter()
            .filter(|actual| actual.eq_ignore_ascii_case(name));
        let Some(only) = candidates.next() else {
            return Ok(None);
        };
        Ok(candidates.next().is_none().then(|| only.clone()))
    }
}

fn key(name: &OsStr) -> String {
    name.to_string_lossy().to_ascii_lowercase()
}

fn list(folder: &Path) -> Listing {
    let folder = if folder.as_os_str().is_empty() {
        Path::new(".")
    } else {
        folder
    };
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Listing::Absent,
        Err(error) => return Listing::Failed(CachedIoError::new(error)),
    };
    collect_names(entries.map(|entry| entry.map(|entry| entry.file_name())))
}

fn collect_names(entries: impl IntoIterator<Item = io::Result<OsString>>) -> Listing {
    let mut names = Names::new();
    for item in entries {
        let actual = match item {
            Ok(name) => name,
            Err(error) => return Listing::Failed(CachedIoError::new(error)),
        };
        names.entry(key(&actual)).or_default().push(actual);
    }
    Listing::Names(names)
}

/// Retains the OS code or shared original cause for repeated failed lookups.
#[derive(Debug)]
pub(crate) enum CachedIoError {
    Os(i32),
    Other(Arc<io::Error>),
}

impl CachedIoError {
    pub(crate) fn new(error: io::Error) -> Self {
        error
            .raw_os_error()
            .map_or_else(|| Self::Other(Arc::new(error)), Self::Os)
    }

    pub(crate) fn replay(&self) -> io::Error {
        match self {
            Self::Os(code) => io::Error::from_raw_os_error(*code),
            Self::Other(error) => io::Error::new(error.kind(), Self::Other(Arc::clone(error))),
        }
    }
}

impl fmt::Display for CachedIoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Os(code) => fmt::Display::fmt(&io::Error::from_raw_os_error(*code), formatter),
            Self::Other(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl Error for CachedIoError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Os(_) => None,
            Self::Other(error) => Some(error.as_ref()),
        }
    }
}

#[cfg(test)]
#[path = "disk_case_tests.rs"]
mod tests;
