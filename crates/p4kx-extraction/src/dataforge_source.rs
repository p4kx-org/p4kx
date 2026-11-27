// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use p4kx_archive::Archive;

use crate::{
    ArchiveWatch, DataForgeEntry, DataForgeLoadError, DataForgeReadOptions, LoadedDataForge,
};

/// Where a DataForge is read from: the DataForge Entry of an Archive, or a DataForge file on disk
/// such as a `Game2.dcb` kept from an earlier patch.
///
/// The `p4kx dcb` commands, the DataForge tools of the MCP server and the window read a DataForge
/// through this one type.
#[derive(Debug, Clone, Copy)]
pub enum DataForgeSource<'a> {
    /// The DataForge Entry of the Archive, see [`DataForgeEntry::locate`].
    Archive(&'a Archive),
    /// The DataForge file at the path.
    File(&'a Path),
}

impl DataForgeSource<'_> {
    /// Reads the DataForge into memory, as [`DataForgeEntry::read`] or
    /// [`DataForgeEntry::read_file`] does.
    ///
    /// # Errors
    ///
    /// Returns the [`DataForgeLoadError`] of that read.
    pub fn read(self) -> Result<DataForgeEntry, DataForgeLoadError> {
        self.read_until(&|| false)
    }

    /// Reads the DataForge like [`read`](Self::read), polling `stop` before each mebibyte.
    ///
    /// # Errors
    ///
    /// The same as [`read`](Self::read), and [`DataForgeLoadError::Cancelled`] once `stop`
    /// returned `true`.
    pub fn read_until(self, stop: &dyn Fn() -> bool) -> Result<DataForgeEntry, DataForgeLoadError> {
        self.read_with_options(DataForgeReadOptions::default(), stop)
    }

    /// Reads the selected DataForge with caller-selected decoded input admission.
    ///
    /// # Errors
    ///
    /// Returns the [`DataForgeLoadError`] of that read.
    pub fn read_with_options(
        self,
        options: DataForgeReadOptions,
        stop: &dyn Fn() -> bool,
    ) -> Result<DataForgeEntry, DataForgeLoadError> {
        match self {
            Self::Archive(archive) => {
                DataForgeEntry::read_until_with_options(archive, options, stop)
            }
            Self::File(file) => DataForgeEntry::read_file_until_with_options(file, options, stop),
        }
    }

    /// Reads the DataForge and opens it on a thread of its own, as [`LoadedDataForge::load`] or
    /// [`LoadedDataForge::load_file`] does.
    ///
    /// # Errors
    ///
    /// Returns the [`DataForgeLoadError`] of that load.
    pub fn load(self) -> Result<LoadedDataForge, DataForgeLoadError> {
        match self {
            Self::Archive(archive) => LoadedDataForge::load(archive),
            Self::File(file) => LoadedDataForge::load_file(file),
        }
    }

    /// Starts watching the file the DataForge is read from for a change on disk.
    ///
    /// # Errors
    ///
    /// Returns the error of looking at a DataForge file; an Archive always has a watch.
    pub fn watch(self) -> io::Result<ArchiveWatch> {
        match self {
            Self::Archive(archive) => Ok(ArchiveWatch::new(archive)),
            Self::File(file) => ArchiveWatch::of_file(file),
        }
    }

    /// Returns the name the DataForge is shown under: the path of its Entry in the Archive, or the
    /// file name of a DataForge file; `None` when an Archive holds no DataForge.
    #[must_use]
    pub fn name(self) -> Option<String> {
        match self {
            Self::Archive(archive) => {
                DataForgeEntry::locate(archive).map(|entry| entry.path().to_owned())
            }
            Self::File(file) => file
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()),
        }
    }
}
