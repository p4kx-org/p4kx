// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use same_file::Handle;

use crate::launcher_log::LAUNCHER_LOG_FILES;
use crate::wine_context::WineContext;

/// The opened source identity and topology of one launcher discovery snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LauncherStamp {
    pub(crate) roots: Vec<PathBuf>,
    pub(crate) libraries: Vec<LibraryStamp>,
    pub(crate) sources: Vec<SourceStamp>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LibraryStamp {
    pub(crate) path: PathBuf,
    pub(crate) canonical: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceStamp {
    pub(crate) folder: PathBuf,
    pub(crate) prefix: Option<Arc<WineContext>>,
    pub(crate) files: [LogFileStamp; LAUNCHER_LOG_FILES.len()],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LogFileStamp {
    Missing,
    NonRegular,
    Unavailable,
    Regular {
        identity: Arc<Handle>,
        length: u64,
        modified: SystemTime,
    },
}
