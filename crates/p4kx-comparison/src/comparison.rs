// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::AtomicBool;

use p4kx_channels::PublicPatches;
use p4kx_extraction::{
    ArchiveWriteProtection, CancelToken, ConversionError, write_replacing_protected,
};
use serde::{Serialize, Serializer};

use crate::entry_change::{count, count_by_directory};
use crate::{
    Archive, Build, DirectoryCounts, EntryChange, EntryCounts, Levels, RecordChange, RecordCounts,
    Report, report::to_report,
};

/// One of the two Archives of a Comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Side<'a> {
    archive: &'a Path,
    channel: Option<String>,
    build: Option<&'a Build>,
    public_patch: Option<String>,
}

impl<'a> Side<'a> {
    /// Returns the side of `archive`, whose Build has no public patch yet.
    #[must_use]
    pub fn of(archive: &'a Archive) -> Self {
        Self {
            archive: archive.path(),
            channel: p4kx_channels::channel_name(archive.path()),
            build: archive.build(),
            public_patch: None,
        }
    }

    /// Returns the side with the public patch `patches` knows for its Build, if any.
    #[must_use]
    pub fn with_public_patches(mut self, patches: Option<&PublicPatches>) -> Self {
        self.public_patch = self
            .build
            .and_then(|build| patches?.of(build))
            .map(str::to_owned);
        self
    }

    /// Returns the public patch version of the Build, e.g. `4.7.2`, or `None` when unknown.
    #[must_use]
    pub fn public_patch(&self) -> Option<&str> {
        self.public_patch.as_deref()
    }

    /// Returns the path the Archive was opened from.
    #[must_use]
    pub const fn archive(&self) -> &'a Path {
        self.archive
    }

    /// Returns the Channel: the name of the folder holding the Archive.
    #[must_use]
    pub fn channel(&self) -> Option<&str> {
        self.channel.as_deref()
    }

    /// Returns the Build, or `None` when it is unknown.
    #[must_use]
    pub const fn build(&self) -> Option<&'a Build> {
        self.build
    }
}

/// The result of comparing an old with a new Archive.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison<'a> {
    pub(crate) old: Side<'a>,
    pub(crate) new: Side<'a>,
    pub(crate) levels: Levels,
    pub(crate) filter: Option<String>,
    pub(crate) entries: Vec<EntryChange<'a>>,
    pub(crate) records: Vec<RecordChange<'a>>,
}

impl<'a> Comparison<'a> {
    /// Returns the old side.
    #[must_use]
    pub const fn old_side(&self) -> &Side<'a> {
        &self.old
    }

    /// Returns the new side.
    #[must_use]
    pub const fn new_side(&self) -> &Side<'a> {
        &self.new
    }

    /// Returns the levels that were compared.
    #[must_use]
    pub const fn levels(&self) -> Levels {
        self.levels
    }

    /// Returns the text of the Filter that selected the Entries and Records, `None` when
    /// everything was compared.
    #[must_use]
    pub fn filter(&self) -> Option<&str> {
        self.filter.as_deref()
    }

    /// Returns the added, removed and changed Entries, ordered by normalised path.
    #[must_use]
    pub fn entry_changes(&self) -> &[EntryChange<'a>] {
        &self.entries
    }

    /// Returns the number of changed Entries per change kind.
    #[must_use]
    pub fn entry_counts(&self) -> EntryCounts {
        count(&self.entries)
    }

    /// Returns the number of changed Entries per change kind below each top-level directory that
    /// has a change, ordered by normalised directory.
    #[must_use]
    pub fn directory_counts(&self) -> Vec<DirectoryCounts<'a>> {
        count_by_directory(&self.entries)
    }

    /// Returns whether any compared level found a difference.
    #[must_use]
    pub const fn has_differences(&self) -> bool {
        !self.entries.is_empty() || !self.records.is_empty()
    }

    /// Returns the added, removed, moved and changed Records, ordered by record path ignoring case
    /// and separator kind, then by GUID.
    #[must_use]
    pub fn record_changes(&self) -> &[RecordChange<'a>] {
        &self.records
    }

    /// Returns the number of changed Records per change kind.
    #[must_use]
    pub fn record_counts(&self) -> RecordCounts {
        RecordCounts::of(&self.records)
    }

    /// Returns the JSON report of the Comparison.
    #[must_use]
    pub fn report(&self) -> Report<'_> {
        to_report(self)
    }

    /// Atomically writes the JSON report as pretty JSON with a final newline.
    ///
    /// # Errors
    /// Returns an I/O error or a permission error retaining the source/target protection refusal.
    pub fn save_report(&self, path: &Path) -> io::Result<()> {
        self.save_report_with_cancel(path, None)
    }

    /// Atomically writes the report, keeping the target unchanged when cancelled.
    ///
    /// # Errors
    /// Returns an I/O or protection error, or `Interrupted` when cancelled.
    pub fn save_report_with_cancel(
        &self,
        path: &Path,
        cancel: Option<&CancelToken>,
    ) -> io::Result<()> {
        static NEVER_CANCELLED: AtomicBool = AtomicBool::new(false);
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(io::Error::from(io::ErrorKind::Interrupted));
        }
        let flag = cancel.map_or(&NEVER_CANCELLED, CancelToken::flag);
        let protection = ArchiveWriteProtection::for_sources(
            &[self.old_side().archive(), self.new_side().archive()],
            flag,
        )
        .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
        write_replacing_protected(path, Some(&protection), cancel, |writer| {
            serde_json::to_writer_pretty(&mut *writer, self)
                .map_err(io::Error::other)
                .and_then(|()| writeln!(writer))
                .map_err(|source| ConversionError::Write {
                    path: path.to_path_buf(),
                    source,
                })?;
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(ConversionError::Cancelled);
            }
            Ok(())
        })
        .map_err(|error| match error {
            ConversionError::Installation(error) => {
                io::Error::new(io::ErrorKind::PermissionDenied, error)
            }
            ConversionError::Cancelled => io::Error::from(io::ErrorKind::Interrupted),
            ConversionError::Write { source, .. } => source,
            error => io::Error::other(error),
        })
    }
}

impl Serialize for Comparison<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.report().serialize(serializer)
    }
}
