// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, Entry};

use crate::entry_path::same_entry_path;
use crate::texture_job::{self, Role};
use crate::{PathIndexBuildError, PathIndexError, path_index, path_key_sort};

type Key = (u64, u32);

/// A physical DDS-family candidate without a read or uniqueness claim.
#[derive(Debug)]
#[non_exhaustive]
pub struct TextureMemberCandidate<'a> {
    /// The actual directory Entry, preserving its original index and declared size.
    pub entry: &'a Entry,
    /// Proved UTF-8 family syntax or an explicitly unproved raw path.
    pub relation: TextureMemberRelation<'a>,
}

/// Whether a physical candidate's family relation is established by its actual path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextureMemberRelation<'a> {
    /// The lossless path matches the queried family under the existing ASCII/separator rule.
    Matched {
        /// The role parsed from the candidate's own path.
        role: Role<'a>,
    },
    /// A display-name coincidence cannot establish the original raw path's membership.
    UnsupportedRawPath,
}

/// Complete family-candidate hashes for one immediately owning Archive.
#[derive(Debug)]
pub struct TextureMemberKeys {
    keys: Vec<Key>,
}

/// A borrowed family index over the Archive its keys were built from.
#[derive(Debug)]
pub struct TextureMemberIndex<'a> {
    archive: &'a Archive,
    keys: &'a [Key],
}

impl TextureMemberKeys {
    /// Builds all DDS-family candidates in one fallible, checkpointed directory pass.
    ///
    /// # Errors
    /// Returns the original admission failure, allocation error or invalid physical position.
    pub fn try_new_with<E>(
        archive: &Archive,
        mut check: impl FnMut(u64) -> Result<(), E>,
    ) -> Result<Self, PathIndexBuildError<E>> {
        check(0).map_err(PathIndexBuildError::Interrupted)?;
        let bound = archive.entries().len();
        if let Some(last) = bound.checked_sub(1) {
            u32::try_from(last).map_err(|_| PathIndexError::IndexWidth { position: last })?;
        }
        let mut keys = Vec::new();
        let mut requested = 0;
        for (position, entry) in archive.entries().iter().enumerate() {
            check(1).map_err(PathIndexBuildError::Interrupted)?;
            let Some((header, _)) = family(entry.path()) else {
                continue;
            };
            let hash = path_index::try_path_key(header, &mut check)?;
            reserve(&mut keys, &mut requested, bound, &mut check)?;
            let physical =
                u32::try_from(position).map_err(|_| PathIndexError::IndexWidth { position })?;
            keys.push((hash, physical));
        }
        path_key_sort::sort(&mut keys, &mut check)?;
        check(0).map_err(PathIndexBuildError::Interrupted)?;
        Ok(Self { keys })
    }

    /// Lends the keys over the same Archive, without cloning paths or parsed headers.
    ///
    /// These keys must have been built from `archive`; inventory completeness stays caller-owned.
    #[must_use]
    pub fn index<'a>(&'a self, archive: &'a Archive) -> TextureMemberIndex<'a> {
        TextureMemberIndex {
            archive,
            keys: &self.keys,
        }
    }
}

impl<'a> TextureMemberIndex<'a> {
    /// Visits every family candidate in physical order, including duplicates, gaps and orphans.
    ///
    /// Unsupported raw paths are unproved candidates and cannot establish unique or missing data.
    ///
    /// # Errors
    /// Returns original hashing/search/comparison admission failures or a mismatched position.
    pub fn family_candidates_with<'q, E: 'q>(
        &'q self,
        header_path: &'q str,
        mut check: impl FnMut(u64) -> Result<(), E> + 'q,
    ) -> Result<
        impl Iterator<Item = Result<TextureMemberCandidate<'a>, PathIndexBuildError<E>>> + 'q,
        PathIndexBuildError<E>,
    > {
        let key = path_index::try_path_key(header_path, &mut check)?;
        Ok(path_index::matching_positions_with(self.keys, key, check)?
            .map(move |position| {
                position.and_then(|position| candidate(self.archive, position, header_path))
            })
            .filter_map(|result| match result {
                Ok(Some(candidate)) => Some(Ok(candidate)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            }))
    }
}

fn reserve<E>(
    keys: &mut Vec<Key>,
    requested: &mut usize,
    bound: usize,
    check: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<(), PathIndexBuildError<E>> {
    if keys.len() < *requested {
        return Ok(());
    }
    check(0).map_err(PathIndexBuildError::Interrupted)?;
    let next = if *requested == 0 {
        1
    } else {
        requested.saturating_mul(2)
    }
    .min(bound);
    let additional = next
        .checked_sub(keys.len())
        .filter(|&count| count > 0)
        .ok_or(PathIndexError::SourceMismatch)?;
    keys.try_reserve_exact(additional)
        .map_err(PathIndexError::Allocation)?;
    *requested = next;
    check(0).map_err(PathIndexBuildError::Interrupted)?;
    Ok(())
}

fn family(path: &str) -> Option<(&str, Role<'_>)> {
    let role = texture_job::role(path)?;
    let header = match role {
        Role::Header => path,
        Role::Member(header, _) => header,
    };
    Some((header, role))
}

fn candidate<'a, E>(
    archive: &'a Archive,
    position: u32,
    header_path: &str,
) -> Result<Option<TextureMemberCandidate<'a>>, PathIndexBuildError<E>> {
    let position = usize::try_from(position).map_err(|_| PathIndexError::SourceMismatch)?;
    let entry = archive
        .entries()
        .get(position)
        .ok_or(PathIndexError::SourceMismatch)?;
    let Some((header, role)) = family(entry.path()) else {
        return Ok(None);
    };
    if !same_entry_path(header, header_path) {
        return Ok(None);
    }
    let relation = if archive.raw_path(entry.index()).is_some() {
        TextureMemberRelation::UnsupportedRawPath
    } else {
        TextureMemberRelation::Matched { role }
    };
    Ok(Some(TextureMemberCandidate { entry, relation }))
}

#[cfg(test)]
#[path = "texture_member_keys_tests.rs"]
mod tests;
