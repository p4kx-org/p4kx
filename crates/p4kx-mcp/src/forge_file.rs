// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use p4kx_extraction::{DataForge, DataForgeOrigin, DataForgeSource, FileStamp};

use crate::archives::Opened;
use crate::error::ToolError;
use crate::forge_state::ForgeState;
use crate::query_selection_sources::SelectionReceipt;

/// A DataForge file on disk, such as a `Game2.dcb`, with its DataForge read on first use and
/// kept while the file stays unchanged.
#[derive(Debug)]
pub(crate) struct ForgeFile {
    path: PathBuf,
    stamp: Option<FileStamp>,
    forge: ForgeState,
}

impl ForgeFile {
    /// Creates the handle of the file at `path`, whose stamp was `stamp` when it was looked at,
    /// or `None` when it could not be.
    pub(crate) fn new(path: PathBuf, stamp: Option<FileStamp>) -> Self {
        Self {
            path,
            stamp,
            forge: ForgeState::default(),
        }
    }

    /// Returns whether the file on disk is still the one this handle was made for.
    pub(crate) fn is_stamp(&self, stamp: Option<FileStamp>) -> bool {
        self.stamp == stamp
    }

    fn forge_source(&self) -> DataForgeSource<'_> {
        DataForgeSource::File(&self.path)
    }
}

/// The input of a DataForge tool: an opened Archive or a DataForge file.
#[derive(Debug, Clone)]
pub(crate) enum ForgeSource {
    /// The DataForge Entry of an Archive.
    Archive(Arc<Opened>),
    /// A DataForge file on disk.
    File(Arc<ForgeFile>),
}

impl ForgeSource {
    /// Returns the path, size and modification time of the input as it was opened, which bind a
    /// cursor to this version of it.
    pub(crate) fn version(&self) -> (&Path, u64, Option<SystemTime>) {
        match self {
            Self::Archive(opened) => opened.version(),
            Self::File(file) => {
                let (len, modified) = file
                    .stamp
                    .map_or((0, None), |stamp| (stamp.size(), stamp.modified()));
                (&file.path, len, modified)
            }
        }
    }

    /// Runs `work` against the DataForge with the input path as text, reading the DataForge on
    /// Runs work against the DataForge with the original input kept by its worker.
    pub(crate) fn with_dataforge_origin<T, F>(&self, work: F) -> Result<T, ToolError>
    where
        T: Send + 'static,
        F: FnOnce(&DataForge<'_>, String, &DataForgeOrigin) -> Result<T, ToolError>
            + Send
            + 'static,
    {
        match self {
            Self::Archive(opened) => opened.with_dataforge_origin(work),
            Self::File(file) => file.forge.with_origin(
                || file.forge_source().load(),
                file.path.display().to_string(),
                work,
            ),
        }
    }

    /// Reads the DataForge if that has not happened yet, without running any work on it.
    pub(crate) fn load_dataforge(&self) -> Result<(), ToolError> {
        match self {
            Self::Archive(opened) => opened.load_dataforge(),
            Self::File(file) => file.forge.load(|| file.forge_source().load()),
        }
    }

    /// Returns the selection of the last Query run if it was of `query`.
    pub(crate) fn selection(&self, query: &str) -> Option<Arc<SelectionReceipt>> {
        match self {
            Self::Archive(opened) => opened.selection(query),
            Self::File(file) => file.forge.selection(query),
        }
    }

    /// Keeps `records` as the selection of the last Query run, which was of `query`.
    pub(crate) fn keep_selection(&self, query: &str, records: Arc<SelectionReceipt>) {
        match self {
            Self::Archive(opened) => opened.keep_selection(query, records),
            Self::File(file) => file.forge.keep_selection(query, records),
        }
    }
}
