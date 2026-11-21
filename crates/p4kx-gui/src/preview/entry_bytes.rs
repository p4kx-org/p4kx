// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};

use p4kx_extraction::{Archive, Entry, ObservedEntry};

use super::PREVIEW_SIZE_LIMIT;
use crate::EntryIndex;
use crate::model::describe;

/// Why an Entry was not read whole.
#[derive(Debug)]
pub(super) enum Unreadable {
    TooLarge,
    ArchiveChanged,
    Failed(Option<String>),
}

/// Reads the whole Entry `id` of `index`, up to [`PREVIEW_SIZE_LIMIT`], failing once `dropped`
/// is set or the Archive changed on disk.
pub(super) fn read_whole(
    index: &EntryIndex,
    id: u32,
    dropped: &AtomicBool,
) -> Result<Vec<u8>, Unreadable> {
    read(index, id, dropped).map(|read| read.bytes)
}

/// Reads an Entry and returns the binding of the physical source opened for it.
pub(super) fn read_whole_observed<'index>(
    index: &'index EntryIndex,
    id: u32,
    dropped: &AtomicBool,
) -> Result<(Vec<u8>, Option<ObservedEntry<'index>>), Unreadable> {
    let read = read(index, id, dropped)?;
    Ok((read.bytes, read.archive.observed_entry(read.entry)))
}

struct EntryBytes<'index> {
    bytes: Vec<u8>,
    archive: &'index Archive,
    entry: &'index Entry,
}

fn read<'index>(
    index: &'index EntryIndex,
    id: u32,
    dropped: &AtomicBool,
) -> Result<EntryBytes<'index>, Unreadable> {
    let entry = index.entry(id).ok_or(Unreadable::Failed(None))?;
    if index.watch().check().is_err() {
        return Err(Unreadable::ArchiveChanged);
    }
    if entry.uncompressed_size() > PREVIEW_SIZE_LIMIT {
        return Err(Unreadable::TooLarge);
    }
    let failed = |error: &dyn std::error::Error| Unreadable::Failed(Some(describe(error)));
    let archive = index.entries_archive().map_err(|error| failed(&error))?;
    let reader = archive.open_entry(entry).map_err(|error| failed(&error))?;
    let capacity = usize::try_from(entry.uncompressed_size()).unwrap_or(0);
    let mut bytes = Vec::with_capacity(capacity);
    Cancellable {
        inner: reader,
        dropped,
    }
    .take(PREVIEW_SIZE_LIMIT + 1)
    .read_to_end(&mut bytes)
    .map_err(|error| failed(&error))?;
    if bytes.len() as u64 > PREVIEW_SIZE_LIMIT {
        return Err(Unreadable::TooLarge);
    }
    if index.watch().check().is_err() {
        return Err(Unreadable::ArchiveChanged);
    }
    Ok(EntryBytes {
        bytes,
        archive,
        entry,
    })
}

/// A reader that fails once the preview it reads for was dropped.
struct Cancellable<'a, R> {
    inner: R,
    dropped: &'a AtomicBool,
}

impl<R: Read> Read for Cancellable<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.dropped.load(Ordering::Acquire) {
            return Err(std::io::Error::other("the preview was closed"));
        }
        self.inner.read(buf)
    }
}
