// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};

use p4kx_archive::{Archive, ArchiveError, Entry};
use p4kx_cryxml::CryXmlError;
use rayon::prelude::*;
use thiserror::Error;

use crate::entry_path::same_entry_path;

const MAX_RESERVED_LEN: u64 = 1 << 20;

/// A failure to read the content of an Entry.
#[derive(Debug, Error)]
pub enum EntryReadError {
    /// The Entry cannot be opened.
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    /// The Entry data cannot be read or decoded.
    #[error("cannot read the Entry data")]
    Read(#[source] io::Error),
    /// The offset lies behind the end of the content.
    #[error("the offset {offset} lies beyond the end at {size}")]
    OffsetBeyondEnd {
        /// The requested offset.
        offset: u64,
        /// The size of the content.
        size: u64,
    },
    /// The Entry holds binary data, not text.
    #[error("the Entry holds binary data, not text")]
    Binary,
    /// The Entry, or the text it decodes to, is too large to decode as a whole.
    #[error("the Entry is too large to decode as a whole: more than {limit} bytes")]
    TooLarge {
        /// The limit in bytes that was exceeded.
        limit: u64,
    },
    /// The `CryXml` is corrupt.
    #[error("cannot convert the CryXml")]
    Convert(#[source] CryXmlError),
}

/// A range of the decompressed and decrypted data of an Entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryBytes {
    /// The offset of the first byte in the Entry.
    pub offset: u64,
    /// The bytes read.
    pub bytes: Vec<u8>,
    /// The size of the Entry.
    pub size: u64,
}

impl EntryBytes {
    /// Returns the offset of the next range, or `None` when the range reaches the end.
    #[must_use]
    pub fn next_offset(&self) -> Option<u64> {
        let end = self.offset.saturating_add(self.bytes.len() as u64);
        (end < self.size).then_some(end)
    }
}

/// Returns the Entry of `archive` named `path`, ignoring ASCII case and the separator kind.
#[must_use]
pub fn find_entry<'a>(archive: &'a Archive, path: &str) -> Option<&'a Entry> {
    archive
        .entries()
        .par_iter()
        .find_first(|entry| same_entry_path(entry.path(), path))
}

/// Reads at most `max_len` bytes of the data of `entry` from `offset`.
///
/// # Errors
///
/// Returns [`EntryReadError::OffsetBeyondEnd`] when `offset` exceeds the Entry size, and another
/// [`EntryReadError`] when the data cannot be read.
pub fn read_bytes(
    archive: &Archive,
    entry: &Entry,
    offset: u64,
    max_len: usize,
) -> Result<EntryBytes, EntryReadError> {
    let size = entry.uncompressed_size();
    let remaining = size
        .checked_sub(offset)
        .ok_or(EntryReadError::OffsetBeyondEnd { offset, size })?;
    let mut reader = archive.open_entry_at(entry, offset)?;
    let bytes = read_up_to(&mut reader, remaining.min(max_len as u64))?;
    Ok(EntryBytes {
        offset,
        bytes,
        size,
    })
}

pub(crate) fn read_up_to(reader: &mut impl Read, len: u64) -> Result<Vec<u8>, EntryReadError> {
    let mut bytes = Vec::with_capacity(usize::try_from(len.min(MAX_RESERVED_LEN)).unwrap_or(0));
    reader
        .by_ref()
        .take(len)
        .read_to_end(&mut bytes)
        .map_err(EntryReadError::Read)?;
    Ok(bytes)
}
