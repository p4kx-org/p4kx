// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use thiserror::Error;

use crate::Entry;
use crate::archive_metadata::ArchiveMetadata;

/// How many Entry issues [`Archive::issues`](crate::Archive::issues) lists at most;
/// [`Archive::issue_count`](crate::Archive::issue_count) still counts all of them.
pub const MAX_ENTRY_ISSUES: usize = 1024;

/// A defect of one Entry that does not keep the Archive from opening.
///
/// The Entry stays listed. Reading it fails with an [`ArchiveError`](crate::ArchiveError) only
/// where the defect makes its data unreadable. An Entry has at most one issue of each kind.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum EntryIssue {
    /// The Entry's data ends behind the start of the trailer or beyond the file; reading it fails.
    #[error("its data lies outside the file")]
    DataOutsideFile,
    /// The encryption field is neither 0 nor 1; the Entry is read as encrypted.
    #[error("its encryption field is {value}, neither 0 nor 1; it is read as encrypted")]
    UnknownEncryption {
        /// The value of the field.
        value: u16,
    },
    /// The CIG encryption block is not 6 bytes long; its value is read as far as it goes.
    #[error("its encryption block is {size} bytes long, not 6")]
    EncryptionBlockSize {
        /// The block length as stored, header included.
        size: u16,
    },
    /// A CIG hash block is not 36 bytes long, 4 header and 32 digest; that block states no
    /// SHA-256. A later block of the record may still state one.
    #[error("a hash block is {size} bytes long, not 36; it states no SHA-256")]
    HashBlockSize {
        /// The block length as stored, header included.
        size: u16,
    },
    /// The path is not UTF-8; [`Entry::path`] holds it with invalid bytes replaced, and
    /// [`Archive::raw_path`](crate::Archive::raw_path) returns the stored bytes.
    #[error("its path is not UTF-8")]
    PathNotUtf8,
    /// The Archive was opened with recovery and its Entries come from a scan for central-directory
    /// records, not from the end records. Listed once, at Entry 0, so the directory may be
    /// incomplete: Entries whose records are lost are missing.
    #[error("the Archive was recovered by scanning for directory records; Entries may be missing")]
    Recovered,
    /// Recovery stopped at a limit (the Entry count, the signature hits or the work budget), so
    /// more Entries than the missing ones of [`EntryIssue::Recovered`] may be absent. Listed at
    /// Entry 0 after it.
    #[error("recovery stopped at a scan limit; more Entries may be missing")]
    RecoveryLimit,
}

impl EntryIssue {
    /// Returns whether the issue concerns the whole Archive, not the Entry it is listed at.
    #[must_use]
    pub const fn is_archive_wide(&self) -> bool {
        matches!(self, Self::Recovered | Self::RecoveryLimit)
    }

    const fn kind(&self) -> u8 {
        match self {
            Self::DataOutsideFile => 1,
            Self::UnknownEncryption { .. } => 2,
            Self::EncryptionBlockSize { .. } => 4,
            Self::PathNotUtf8 => 8,
            Self::HashBlockSize { .. } => 16,
            Self::Recovered => 32,
            Self::RecoveryLimit => 64,
        }
    }
}

/// The defects found while reading a directory: the first [`MAX_ENTRY_ISSUES`] in Entry order,
/// the exact count, and the stored bytes of every path that is not UTF-8.
#[derive(Debug, Default)]
pub(crate) struct IssueLog {
    pub(crate) listed: Vec<(usize, EntryIssue)>,
    pub(crate) count: usize,
    pub(crate) raw_paths: Vec<(usize, Box<[u8]>)>,
    seen: (usize, u8),
}

impl IssueLog {
    /// Records `issue` for the Entry at `index` unless the Entry has one of its kind already.
    /// Entries must be recorded in order.
    #[cold]
    #[inline(never)]
    pub(crate) fn record(&mut self, index: usize, issue: EntryIssue) {
        if self.seen.0 != index {
            self.seen = (index, 0);
        }
        let kind = issue.kind();
        if self.seen.1 & kind != 0 {
            return;
        }
        self.seen.1 |= kind;
        self.count += 1;
        if self.listed.len() < MAX_ENTRY_ISSUES {
            self.listed.push((index, issue));
        }
    }

    /// Takes over the defects `other` recorded, which carry the Entry positions they were read at.
    pub(crate) fn absorb(&mut self, other: Self) {
        for (index, issue) in other.listed {
            self.record(index, issue);
        }
        self.raw_paths.extend(other.raw_paths);
    }

    /// Lists `issue` for Entry 0 before all others.
    pub(crate) fn record_first(&mut self, issue: EntryIssue) {
        self.count += 1;
        self.listed.insert(0, (0, issue));
        self.listed.truncate(MAX_ENTRY_ISSUES);
    }

    /// Returns the stored path bytes of the Entry at `index` when its path is not UTF-8.
    pub(crate) fn raw_path(&self, index: usize) -> Option<&[u8]> {
        self.raw_paths
            .binary_search_by_key(&index, |(at, _)| *at)
            .ok()
            .and_then(|position| self.raw_paths.get(position))
            .map(|(_, raw)| &**raw)
    }
}

/// Decodes the stored path bytes of the Entry at `index`; a path that is not UTF-8 is decoded
/// lossily and recorded in `log` with its bytes.
pub(crate) fn decode_path<'a>(bytes: &'a [u8], index: usize, log: &mut IssueLog) -> Cow<'a, str> {
    std::str::from_utf8(bytes).map_or_else(|_| lossy_path(bytes, index, log), Cow::Borrowed)
}

#[cold]
#[inline(never)]
fn lossy_path<'a>(bytes: &'a [u8], index: usize, log: &mut IssueLog) -> Cow<'a, str> {
    log.raw_paths.push((index, bytes.into()));
    log.record(index, EntryIssue::PathNotUtf8);
    String::from_utf8_lossy(bytes)
}

/// The Entries of a directory and the defects found while reading it.
pub(crate) struct Directory {
    pub(crate) entries: Vec<Entry>,
    pub(crate) log: IssueLog,
    pub(crate) installation_flags: Option<u8>,
    pub(crate) metadata: ArchiveMetadata,
}
