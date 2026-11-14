// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use crate::disk_case::{CachedIoError, CaseIndex};
use crate::{ConversionError, Texture};

/// Disk textures sharing case listings and chosen input paths for one conversion run.
///
/// New handles refresh input selection; cloned handles and opened textures retain the same run.
#[derive(Clone, Debug, Default)]
pub struct DiskTextures(Arc<Mutex<DiskFileIndex>>);

impl DiskTextures {
    /// Creates an empty conversion run.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens an explicitly named header with linked colour and alpha parts from this run.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`Texture::from_file`].
    pub fn open(&self, path: &Path) -> Result<Texture<'static>, ConversionError> {
        Texture::from_disk(self, path)
    }

    pub(crate) fn remember_exact(&self, path: &Path) -> io::Result<()> {
        let path = std::path::absolute(path)?;
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .bindings
            .entry(path)
            .or_insert(Binding::Exact);
        Ok(())
    }

    pub(crate) fn resolve_path(&self, path: &Path) -> io::Result<Option<PathBuf>> {
        self.with_path(path, |actual| Ok(actual.to_path_buf()))
    }

    pub(crate) fn exists(&self, path: &Path) -> io::Result<bool> {
        self.with_path(path, |_| Ok(()))
            .map(|found| found.is_some())
    }

    pub(crate) fn part_len(&self, path: &Path) -> io::Result<Option<u64>> {
        self.with_path(path, |actual| {
            fs::metadata(actual).map(|metadata| metadata.is_file().then_some(metadata.len()))
        })
        .map(Option::flatten)
    }

    pub(crate) fn open_file(&self, path: &Path) -> io::Result<File> {
        self.with_path(path, |actual| File::open(actual))?
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    pub(crate) fn open_file_with_path(&self, path: &Path) -> io::Result<(PathBuf, File)> {
        self.with_path(path, |actual| {
            File::open(actual).map(|file| (actual.to_path_buf(), file))
        })?
        .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn with_path<T>(
        &self,
        path: &Path,
        use_path: impl FnOnce(&Path) -> io::Result<T>,
    ) -> io::Result<Option<T>> {
        let path = std::path::absolute(path)?;
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let result = {
            let DiskFileIndex { case, bindings } = &mut *state;
            let entry = match bindings.entry(path) {
                Entry::Occupied(entry) => entry,
                Entry::Vacant(entry) => {
                    let binding = Binding::find(case, entry.key());
                    entry.insert_entry(binding)
                }
            };
            match entry.get() {
                Binding::Exact => use_path(entry.key()).map(Some),
                Binding::Variant(actual) => use_path(actual).map(Some),
                Binding::Absent => Ok(None),
                Binding::Failed(error) => Err(error.replay()),
            }
        };
        drop(state);
        result
    }
}

#[derive(Debug, Default)]
struct DiskFileIndex {
    case: CaseIndex,
    bindings: HashMap<PathBuf, Binding>,
}

#[derive(Debug)]
enum Binding {
    Exact,
    Variant(PathBuf),
    Absent,
    Failed(CachedIoError),
}

impl Binding {
    fn find(case: &mut CaseIndex, path: &Path) -> Self {
        match Self::lookup(case, path) {
            Ok(binding) => binding,
            Err(error) => Self::Failed(CachedIoError::new(error)),
        }
    }

    fn lookup(case: &mut CaseIndex, path: &Path) -> io::Result<Self> {
        match fs::metadata(path) {
            Ok(metadata) => {
                return Ok(if metadata.is_file() {
                    Self::Exact
                } else {
                    Self::Absent
                });
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let Some(actual) = case.variant(path)? else {
            return Ok(Self::Absent);
        };
        match fs::metadata(&actual) {
            Ok(metadata) if metadata.is_file() => Ok(if actual == path {
                Self::Exact
            } else {
                Self::Variant(actual)
            }),
            Ok(_) => Ok(Self::Absent),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::Absent),
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
#[path = "disk_textures_tests.rs"]
mod tests;
