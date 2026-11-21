// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{PathIndexBuildError, PathIndexError};

type Key = (u64, u32);

const COPY_KEYS: usize = 4096;
const RADIX_BUCKETS: usize = 256;

pub(crate) fn sort<E>(
    keys: &mut [Key],
    check: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<(), PathIndexBuildError<E>> {
    let mut scratch = Vec::new();
    check(0).map_err(PathIndexBuildError::Interrupted)?;
    scratch
        .try_reserve_exact(keys.len())
        .map_err(PathIndexError::Allocation)?;
    scratch.resize(keys.len(), (0, 0));
    for byte in 0..size_of::<u64>() {
        pass(keys, &mut scratch, byte, check)?;
        for (target, source) in keys.chunks_mut(COPY_KEYS).zip(scratch.chunks(COPY_KEYS)) {
            check(1).map_err(PathIndexBuildError::Interrupted)?;
            target.copy_from_slice(source);
        }
    }
    Ok(())
}

fn pass<E>(
    source: &[Key],
    target: &mut [Key],
    byte: usize,
    check: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<(), PathIndexBuildError<E>> {
    let mut positions = [0_usize; RADIX_BUCKETS];
    for &(hash, _) in source {
        check(1).map_err(PathIndexBuildError::Interrupted)?;
        let slot = bucket(hash, byte)?;
        *positions
            .get_mut(slot)
            .ok_or(PathIndexError::SourceMismatch)? += 1;
    }
    let mut next = 0_usize;
    for count in &mut positions {
        let size = *count;
        *count = next;
        next = next
            .checked_add(size)
            .ok_or(PathIndexError::SourceMismatch)?;
    }
    for &entry in source {
        check(1).map_err(PathIndexBuildError::Interrupted)?;
        let slot = bucket(entry.0, byte)?;
        let position = positions
            .get_mut(slot)
            .ok_or(PathIndexError::SourceMismatch)?;
        *target
            .get_mut(*position)
            .ok_or(PathIndexError::SourceMismatch)? = entry;
        *position += 1;
    }
    Ok(())
}

fn bucket<E>(hash: u64, byte: usize) -> Result<usize, PathIndexBuildError<E>> {
    hash.to_le_bytes()
        .get(byte)
        .map(|byte| usize::from(*byte))
        .ok_or(PathIndexError::SourceMismatch)
        .map_err(PathIndexBuildError::Index)
}
