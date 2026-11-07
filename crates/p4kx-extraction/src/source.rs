// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The files a run reads: the Entries of an Archive or the files of a folder.

use std::fmt::Debug;
use std::io::{self, Read};
use std::path::Path;

use p4kx_archive::{Archive, Checksum, Entry};
use rayon::prelude::*;

use crate::ExtractionError;

/// One file of a run, an Entry of an Archive or a file of a folder, named by its path with `\` as
/// separator.
///
/// The file belongs to the source that listed it and is opened through it by its position, so a
/// file can never be handed to another source.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SourceEntry<'a> {
    path: &'a str,
    size: u64,
    index: usize,
    source: &'a dyn EntrySource,
}

impl<'a> SourceEntry<'a> {
    /// Returns the file at `index` of `source`, named `path` and `size` bytes long.
    pub(crate) const fn new(
        source: &'a dyn EntrySource,
        (path, size): (&'a str, u64),
        index: usize,
    ) -> Self {
        Self {
            path,
            size,
            index,
            source,
        }
    }

    /// Returns the Entry `entry` of `archive`.
    pub(crate) fn of_archive(archive: &'a Archive, entry: &'a Entry) -> Self {
        Self::new(
            archive,
            (entry.path(), entry.uncompressed_size()),
            entry.index(),
        )
    }

    /// Returns the path, with `\` as separator.
    pub(crate) const fn path(&self) -> &'a str {
        self.path
    }

    /// Returns the size of the data.
    pub(crate) const fn uncompressed_size(&self) -> u64 {
        self.size
    }

    /// Returns the position in the source the file was listed at.
    pub(crate) const fn index(&self) -> usize {
        self.index
    }

    pub(crate) fn archive_path(&self) -> Option<&Path> {
        self.source.archive_path()
    }

    /// Returns the checksum its source states, or `None` when it states none.
    pub(crate) fn checksum(&self) -> Option<Checksum> {
        self.source.checksum(self.index)
    }

    /// Opens the data of the file, checked against its stated checksum when `verify` is set and
    /// the source states one.
    pub(crate) fn open(&self, verify: bool) -> Result<SourceReader<'a>, ExtractionError> {
        self.source.open(self.index, verify)
    }
}

/// The reader of the data of one file.
pub(crate) type SourceReader<'a> = Box<dyn Read + 'a>;

/// Where a run reads its files from; a file is addressed by its position in the source.
pub(crate) trait EntrySource: Debug + Sync {
    /// Returns the backing Archive's host path when this source is Archive-backed.
    fn archive_path(&self) -> Option<&Path> {
        None
    }
    /// Opens the data of the file at `index`.
    fn open(&self, index: usize, verify: bool) -> Result<SourceReader<'_>, ExtractionError>;

    /// Returns the checksum the source states for the file at `index`.
    fn checksum(&self, index: usize) -> Option<Checksum>;

    /// Returns the path bytes as stored of the file at `index` when its path is not UTF-8.
    fn raw_path(&self, index: usize) -> Option<&[u8]>;

    /// Returns the path of the file at `index`.
    fn path_of(&self, index: usize) -> Option<&str>;

    /// Returns every file whose path `wanted` accepts, in parallel over the source.
    fn members(&self, wanted: &(dyn Fn(&str) -> bool + Sync)) -> Vec<SourceEntry<'_>>;
}

impl EntrySource for Archive {
    fn archive_path(&self) -> Option<&Path> {
        Some(self.path())
    }
    fn open(&self, index: usize, verify: bool) -> Result<SourceReader<'_>, ExtractionError> {
        let entry = self.entries().get(index).ok_or_else(|| {
            ExtractionError::Read(io::Error::new(
                io::ErrorKind::NotFound,
                "the Archive has no Entry at that position",
            ))
        })?;
        let reader = if verify {
            self.open_entry_verified(entry)?
        } else {
            self.open_entry(entry)?
        };
        Ok(Box::new(reader))
    }

    fn checksum(&self, index: usize) -> Option<Checksum> {
        self.entries().get(index).map(Entry::checksum)
    }

    fn raw_path(&self, index: usize) -> Option<&[u8]> {
        Self::raw_path(self, index)
    }

    fn path_of(&self, index: usize) -> Option<&str> {
        self.entries().get(index).map(Entry::path)
    }

    fn members(&self, wanted: &(dyn Fn(&str) -> bool + Sync)) -> Vec<SourceEntry<'_>> {
        self.entries()
            .par_iter()
            .filter(|entry| wanted(entry.path()))
            .map(|entry| SourceEntry::of_archive(self, entry))
            .collect()
    }
}
