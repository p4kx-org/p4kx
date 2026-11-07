// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_archive::{Archive, Entry};
use rayon::prelude::*;

use crate::entry_path::same_entry_path;
use crate::path_key_sort;
use crate::{PathIndexBuildError, PathIndexError};

const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
const HASH_BLOCK_BYTES: usize = 4096;

/// The Entries of an Archive looked up by path, ignoring ASCII case and the separator kind.
///
/// Building the index hashes every path once; each lookup is then a binary search.
#[derive(Debug)]
pub struct PathIndex<'a> {
    archive: &'a Archive,
    keys: Cow<'a, [(u64, u32)]>,
}

/// The sorted path hashes of an Archive's Entries, kept apart from the Archive so one build serves
/// every lookup: [`PathKeys::index`] lends them as a [`PathIndex`].
#[derive(Debug, Clone)]
pub struct PathKeys {
    keys: Vec<(u64, u32)>,
}

impl PathKeys {
    /// Hashes every path of `archive`'s Entries once.
    #[must_use]
    pub fn new(archive: &Archive) -> Self {
        let mut keys: Vec<(u64, u32)> = archive
            .entries()
            .par_iter()
            .enumerate()
            .filter_map(|(position, entry)| {
                Some((path_key(entry.path()), u32::try_from(position).ok()?))
            })
            .collect();
        keys.par_sort_unstable();
        Self { keys }
    }

    /// Builds all keys with fallible storage and caller admission for hashing and sorting.
    ///
    /// # Errors
    /// Returns the original admission error, allocation failure or an invalid physical index.
    pub fn try_new_with<E>(
        archive: &Archive,
        mut check: impl FnMut(u64) -> Result<(), E>,
    ) -> Result<Self, PathIndexBuildError<E>> {
        check(0).map_err(PathIndexBuildError::Interrupted)?;
        if let Some(position) = archive.entries().len().checked_sub(1) {
            u32::try_from(position).map_err(|_| PathIndexError::IndexWidth { position })?;
        }
        let mut keys = Vec::new();
        keys.try_reserve_exact(archive.entries().len())
            .map_err(PathIndexError::Allocation)?;
        for (position, entry) in archive.entries().iter().enumerate() {
            let hash = try_path_key(entry.path(), &mut check)?;
            let physical =
                u32::try_from(position).map_err(|_| PathIndexError::IndexWidth { position })?;
            keys.push((hash, physical));
        }
        path_key_sort::sort(&mut keys, &mut check)?;
        Ok(Self { keys })
    }

    /// Returns the index over `archive`, which must be the Archive these keys were built from; for
    /// another one lookups find nothing or an Entry whose path really matches.
    #[must_use]
    pub fn index<'a>(&'a self, archive: &'a Archive) -> PathIndex<'a> {
        PathIndex {
            archive,
            keys: Cow::Borrowed(&self.keys),
        }
    }
}

impl<'a> PathIndex<'a> {
    /// Builds the index of `archive`'s Entries.
    #[must_use]
    pub fn new(archive: &'a Archive) -> Self {
        Self {
            archive,
            keys: Cow::Owned(PathKeys::new(archive).keys),
        }
    }

    /// Returns the Archive the index is over.
    #[must_use]
    pub const fn archive(&self) -> &'a Archive {
        self.archive
    }

    /// Returns the Entry at `path`, or `None` when the Archive holds none.
    #[must_use]
    pub fn find(&self, path: &str) -> Option<&'a Entry> {
        self.archive.entries().get(self.position(path)?)
    }

    /// Visits all matching Entries in physical order, retaining actual duplicate paths.
    ///
    /// The caller admits hashing, binary search and every visited hash-bucket comparison.
    ///
    /// # Errors
    /// Returns the original admission failure or a mismatched physical index.
    pub fn matching_entries_with<'i, E: 'i>(
        &'i self,
        path: &'i str,
        mut check: impl FnMut(u64) -> Result<(), E> + 'i,
    ) -> Result<
        impl Iterator<Item = Result<&'a Entry, PathIndexBuildError<E>>> + 'i,
        PathIndexBuildError<E>,
    > {
        let key = try_path_key(path, &mut check)?;
        let start = lower_bound(&self.keys, key, &mut check)?;
        Ok(self
            .keys
            .get(start..)
            .unwrap_or_default()
            .iter()
            .scan(false, move |done, &(candidate, position)| {
                if *done {
                    return None;
                }
                if let Err(error) = check(1) {
                    *done = true;
                    return Some(Err(PathIndexBuildError::Interrupted(error)));
                }
                if candidate != key {
                    *done = true;
                    return None;
                }
                Some(match_entry(self.archive, position, path))
            })
            .filter_map(|result| match result {
                Ok(Some(entry)) => Some(Ok(entry)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            }))
    }
    /// Returns the position in [`Archive::entries`] of the first Entry at `path`, or `None`
    /// when the Archive holds none.
    #[must_use]
    pub fn position(&self, path: &str) -> Option<usize> {
        let key = path_key(path);
        let start = self.keys.partition_point(|&(candidate, _)| candidate < key);
        let entries = self.archive.entries();
        self.keys
            .get(start..)
            .unwrap_or_default()
            .iter()
            .take_while(|&&(candidate, _)| candidate == key)
            .filter_map(|&(_, position)| usize::try_from(position).ok())
            .find(|&position| {
                entries
                    .get(position)
                    .is_some_and(|entry| same_entry_path(entry.path(), path))
            })
    }
}

pub(crate) fn path_key(path: &str) -> u64 {
    path.bytes().fold(FNV_OFFSET, fold_path_byte)
}

fn try_path_key<E>(
    path: &str,
    check: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<u64, PathIndexBuildError<E>> {
    let mut hash = FNV_OFFSET;
    for block in path.as_bytes().chunks(HASH_BLOCK_BYTES) {
        check(1).map_err(PathIndexBuildError::Interrupted)?;
        hash = block
            .iter()
            .fold(hash, |hash, byte| fold_path_byte(hash, *byte));
    }
    Ok(hash)
}

fn fold_path_byte(hash: u64, byte: u8) -> u64 {
    let byte = if byte == b'/' {
        b'\\'
    } else {
        byte.to_ascii_lowercase()
    };
    (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
}

fn lower_bound<E>(
    keys: &[(u64, u32)],
    key: u64,
    check: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<usize, PathIndexBuildError<E>> {
    let mut low = 0;
    let mut high = keys.len();
    while low < high {
        check(1).map_err(PathIndexBuildError::Interrupted)?;
        let middle = low + (high - low) / 2;
        let candidate = keys.get(middle).ok_or(PathIndexError::SourceMismatch)?;
        if candidate.0 < key {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    Ok(low)
}

fn match_entry<'a, E>(
    archive: &'a Archive,
    position: u32,
    path: &str,
) -> Result<Option<&'a Entry>, PathIndexBuildError<E>> {
    let position = usize::try_from(position).map_err(|_| PathIndexError::SourceMismatch)?;
    let entry = archive
        .entries()
        .get(position)
        .ok_or(PathIndexError::SourceMismatch)?;
    Ok(same_entry_path(entry.path(), path).then_some(entry))
}

#[cfg(test)]
#[path = "path_index_tests.rs"]
mod tests;
