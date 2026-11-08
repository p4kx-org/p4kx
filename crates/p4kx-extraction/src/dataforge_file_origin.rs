// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::FileStamp;

/// Owned identity and observations of one successfully read standalone DataForge file.
#[derive(Clone)]
pub struct DataForgeFileOrigin {
    source: Arc<FileInput>,
}

struct FileInput {
    path: PathBuf,
    stamp: FileStamp,
}

impl DataForgeFileOrigin {
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
}

impl fmt::Debug for DataForgeFileOrigin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DataForgeFileOrigin")
            .field("stamp", &self.source.stamp)
            .finish_non_exhaustive()
    }
}
