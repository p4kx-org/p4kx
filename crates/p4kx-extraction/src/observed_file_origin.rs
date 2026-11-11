// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::FileStamp;

/// Owned identity and observations of one successfully read standalone file.
#[derive(Clone)]
pub struct ObservedFileOrigin {
    source: Arc<FileInput>,
}

struct FileInput {
    path: PathBuf,
    stamp: FileStamp,
}

impl ObservedFileOrigin {
    pub(crate) fn new(path: PathBuf, stamp: FileStamp) -> Self {
        Self {
            source: Arc::new(FileInput { path, stamp }),
        }
    }

    /// Returns the actual absolute input path for in-process source binding.
    /// The host path is not a publication label.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.source.path
    }

    /// Returns the opened-handle observations accepted by the post-read change check.
    #[must_use]
    pub fn stamp(&self) -> FileStamp {
        self.source.stamp
    }

    /// Returns whether both handles describe the same observed file read.
    #[must_use]
    pub fn is_same_input(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source, &other.source)
    }

    pub(crate) fn of_read(
        path: &Path,
        handle: &File,
        before: FileStamp,
        byte_length: usize,
    ) -> io::Result<Option<Self>> {
        let absolute = std::path::absolute(path)?;
        let after = FileStamp::from_metadata(&handle.metadata()?);
        let stable = byte_length as u64 == before.size()
            && after == before
            && FileStamp::of(&absolute)? == before;
        Ok(stable.then(|| Self::new(absolute, before)))
    }
}

impl fmt::Debug for ObservedFileOrigin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObservedFileOrigin")
            .field("stamp", &self.source.stamp)
            .finish_non_exhaustive()
    }
}

/// Standalone file bytes and their original opened-file observation.
#[derive(Debug)]
pub struct ObservedFileRead {
    bytes: Vec<u8>,
    origin: Option<ObservedFileOrigin>,
}

impl ObservedFileRead {
    /// Returns the original bytes read from the opened file.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the read-bound file observation when the opened file stayed stable.
    #[must_use]
    pub const fn origin(&self) -> Option<&ObservedFileOrigin> {
        self.origin.as_ref()
    }
}

/// Reads a standalone file and retains its original observation when stable.
///
/// # Errors
/// Returns a file-open, metadata or read failure.
pub fn read_observed_file(path: &Path) -> io::Result<ObservedFileRead> {
    let mut file = File::open(path)?;
    let before = FileStamp::from_metadata(&file.metadata()?);
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let origin = ObservedFileOrigin::of_read(path, &file, before, bytes.len())?;
    Ok(ObservedFileRead { bytes, origin })
}
