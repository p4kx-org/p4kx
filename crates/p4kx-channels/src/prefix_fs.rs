// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsString;
use std::fs::{self, File, ReadDir};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// The filesystem kind needed by bounded prefix discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiscoveryPathKind {
    /// A directory, including a resolved named directory alias.
    Directory,
    /// A regular file.
    File,
    /// Another filesystem object.
    Other,
}

/// The filesystem operations available to deterministic prefix discovery.
pub trait PrefixFileSystem {
    /// A directory-name iterator retaining individual entry errors.
    type Entries: Iterator<Item = io::Result<OsString>>;
    /// An opened known Steam configuration file.
    type Config: Read;

    /// Returns the kind of a named path.
    ///
    /// # Errors
    /// Returns the original metadata error, including ordinary absence.
    fn kind(&self, path: &Path) -> io::Result<DiscoveryPathKind>;

    /// Opens a non-recursive directory-name iterator.
    ///
    /// # Errors
    /// Returns the original directory-open error.
    fn read_dir(&self, path: &Path) -> io::Result<Self::Entries>;

    /// Opens one explicitly named Steam library-list configuration.
    ///
    /// # Errors
    /// Returns the original file-open error.
    fn open_config(&self, path: &Path) -> io::Result<Self::Config>;

    /// Resolves one admitted named directory alias.
    ///
    /// # Errors
    /// Returns the original canonicalization error.
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;
}

/// The production adapter for the host filesystem.
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePrefixFileSystem;

/// A native non-recursive directory-name iterator.
pub struct NativeDirectoryEntries(ReadDir);

impl Iterator for NativeDirectoryEntries {
    type Item = io::Result<OsString>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|entry| entry.map(|entry| entry.file_name()))
    }
}

impl PrefixFileSystem for NativePrefixFileSystem {
    type Entries = NativeDirectoryEntries;
    type Config = File;

    fn kind(&self, path: &Path) -> io::Result<DiscoveryPathKind> {
        fs::metadata(path).map(|metadata| {
            if metadata.is_dir() {
                DiscoveryPathKind::Directory
            } else if metadata.is_file() {
                DiscoveryPathKind::File
            } else {
                DiscoveryPathKind::Other
            }
        })
    }

    fn read_dir(&self, path: &Path) -> io::Result<Self::Entries> {
        fs::read_dir(path).map(NativeDirectoryEntries)
    }

    fn open_config(&self, path: &Path) -> io::Result<Self::Config> {
        File::open(path)
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        fs::canonicalize(path)
    }
}
