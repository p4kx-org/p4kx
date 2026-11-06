// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;

use p4kx_foundation::ArchiveKey;

use crate::entry_reader;
use crate::format_error::FormatError;
use crate::issue::{Directory, IssueLog};
use crate::source::Source;
use crate::{Compression, Entry};

/// How many containers deep [`Archive::open_container`](crate::Archive::open_container) opens.
pub const MAX_CONTAINER_DEPTH: usize = 8;
/// How many Entries a container may hold.
pub const MAX_CONTAINER_ENTRIES: usize = 1 << 20;
/// How many bytes the compressed or encrypted containers of one chain may decode to together;
/// they are held in memory.
pub const MAX_CONTAINER_LEN: u64 = 512 * 1024 * 1024;
/// How many times its stored size an Entry of a container may declare as its uncompressed size.
pub const MAX_COMPRESSION_RATIO: u64 = 2048;

/// Returns the bytes of the container held in `entry` and the decoded-size budget left for the
/// containers inside it: a window of `parent` that keeps `budget` when the Entry is stored
/// unencrypted, otherwise its data decoded into memory, which spends its size from `budget`.
pub(crate) fn source_of(
    parent: &Source,
    entry: &Entry,
    budget: u64,
    key: Option<&ArchiveKey>,
) -> Result<(Source, u64), FormatError> {
    if entry.compression == Compression::Store && !entry.encrypted {
        let offset = entry_reader::data_offset(parent, entry)?;
        if entry.compressed_size != entry.uncompressed_size {
            return Err(stored_sizes_differ(offset, entry));
        }
        let window = parent
            .slice(offset, entry.compressed_size)
            .ok_or_else(|| FormatError::corrupt(offset, "Entry data lies outside the file"))?;
        return Ok((window, budget));
    }
    let Some(left) = budget.checked_sub(entry.uncompressed_size) else {
        return Err(FormatError::LimitExceeded(format!(
            "it declares {} bytes, larger than the {budget} bytes its chain of compressed \
             containers may still decode to",
            entry.uncompressed_size
        )));
    };
    let capacity = entry
        .uncompressed_size
        .min(entry.compressed_size.saturating_mul(MAX_COMPRESSION_RATIO));
    let mut bytes = Vec::with_capacity(usize::try_from(capacity).unwrap_or(0));
    entry_reader::open(parent, entry, key)?.read_to_end(&mut bytes)?;
    Ok((Source::memory(bytes), left))
}

/// Checks the Entries of the container in `source` against the limits, leaves out folder
/// Entries and writes their paths with backslashes.
pub(crate) fn admit(directory: Directory, source: &Source) -> Result<Directory, FormatError> {
    let Directory {
        entries,
        log,
        installation_flags,
    } = directory;
    let mut admitted = Vec::with_capacity(entries.len());
    let mut kept = IssueLog::default();
    let mut dropped = 0;
    let mut listed = log.listed.into_iter().peekable();
    let mut raw_paths = log.raw_paths.into_iter().peekable();
    for (position, mut entry) in entries.into_iter().enumerate() {
        let index = admitted.len();
        let folder = entry.path.ends_with(['\\', '/']) && entry.uncompressed_size == 0;
        while let Some((_, issue)) = listed.next_if(|(at, _)| *at == position) {
            if folder {
                dropped += 1;
            } else {
                kept.listed.push((index, issue));
            }
        }
        while let Some((_, raw)) = raw_paths.next_if(|(at, _)| *at == position) {
            if !folder {
                kept.raw_paths.push((index, raw));
            }
        }
        if folder {
            continue;
        }
        check_sizes(&entry)?;
        if entry.path.contains('/') {
            entry.path = entry.path.replace('/', "\\");
        }
        entry.index = u32::try_from(index).unwrap_or(u32::MAX);
        admitted.push(entry);
    }
    kept.count = log.count.saturating_sub(dropped);
    check_disjoint(&admitted, source)?;
    Ok(Directory {
        entries: admitted,
        log: kept,
        installation_flags,
    })
}

fn check_sizes(entry: &Entry) -> Result<(), FormatError> {
    if entry.compression == Compression::Store
        && !entry.encrypted
        && entry.compressed_size != entry.uncompressed_size
    {
        return Err(stored_sizes_differ(entry.data_offset, entry));
    }
    if entry.uncompressed_size > entry.compressed_size.saturating_mul(MAX_COMPRESSION_RATIO) {
        return Err(FormatError::LimitExceeded(format!(
            "Entry {} declares {} bytes from {} stored bytes, beyond the compression ratio limit \
             of {MAX_COMPRESSION_RATIO}",
            entry.path, entry.uncompressed_size, entry.compressed_size
        )));
    }
    Ok(())
}

/// Refuses Entries whose local header and data, from the header to the real end of the data,
/// overlap those of another Entry.
fn check_disjoint(entries: &[Entry], source: &Source) -> Result<(), FormatError> {
    let mut ranges = Vec::with_capacity(entries.len());
    for entry in entries {
        let data_offset = entry_reader::data_offset(source, entry)?;
        ranges.push((entry.data_offset, data_offset + entry.compressed_size));
    }
    ranges.sort_unstable();
    let mut free_from = 0_u64;
    for (start, end) in ranges {
        if start < free_from {
            return Err(FormatError::corrupt(
                start,
                "Entry data overlaps the data of another Entry",
            ));
        }
        free_from = end;
    }
    Ok(())
}

fn stored_sizes_differ(offset: u64, entry: &Entry) -> FormatError {
    FormatError::corrupt(
        offset,
        format!(
            "stored Entry {} declares {} bytes but stores {}",
            entry.path, entry.uncompressed_size, entry.compressed_size
        ),
    )
}
