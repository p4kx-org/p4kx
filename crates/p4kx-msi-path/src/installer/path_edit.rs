// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::installer::fault::Fault;
use crate::installer::lease::Lease;
use crate::installer::raw_path::{self, RawPath};

const SEPARATOR: [u8; 2] = [b';', 0];

/// Proof of the exact before/after value owned by one insertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Insertion {
    pub(crate) before: RawPath,
    pub(crate) after: RawPath,
}

pub(crate) fn insert(before: &RawPath, lease: &Lease) -> Result<Insertion, Fault> {
    let parts = before.split()?;
    if let Some(parts) = &parts
        && locate(parts.body, lease)?.is_some()
    {
        return Err(Fault::ExistingLease);
    }
    let old_size = parts
        .as_ref()
        .map_or(0, |parts| parts.body.len() + SEPARATOR.len());
    let tail_size = parts.as_ref().map_or(2, |parts| parts.tail.len());
    let body_size = lease
        .entry
        .len()
        .checked_add(old_size)
        .ok_or(Fault::InvalidRawPath)?;
    let total_size = body_size
        .checked_add(tail_size)
        .ok_or(Fault::InvalidRawPath)?;
    if total_size > raw_path::MAX_PATH_BYTES {
        return Err(Fault::InvalidRawPath);
    }
    let mut body = Vec::with_capacity(total_size);
    body.extend_from_slice(&lease.entry);
    if let Some(parts) = parts {
        body.extend_from_slice(&SEPARATOR);
        body.extend_from_slice(parts.body);
    }
    Ok(Insertion {
        before: before.clone(),
        after: before.with_body(body)?,
    })
}

pub(crate) fn remove(
    current: &RawPath,
    lease: &Lease,
    insertion: &Insertion,
) -> Result<RawPath, Fault> {
    if current == &insertion.after {
        return Ok(insertion.before.clone());
    }
    let Some(parts) = current.split()? else {
        return Ok(current.clone());
    };
    let body = parts.body;
    let Some(mut range) = locate(body, lease)? else {
        return Ok(current.clone());
    };
    if body.get(range.end..range.end + SEPARATOR.len()) == Some(&SEPARATOR) {
        range.end += SEPARATOR.len();
    } else if range.start >= SEPARATOR.len() {
        range.start -= SEPARATOR.len();
    }
    let mut remaining = body
        .get(..range.start)
        .ok_or(Fault::InvalidRawPath)?
        .to_vec();
    remaining.extend_from_slice(body.get(range.end..).ok_or(Fault::InvalidRawPath)?);
    current.with_body(remaining)
}

pub(crate) fn locate(body: &[u8], lease: &Lease) -> Result<Option<Range<usize>>, Fault> {
    let mut found = None;
    let mut start = 0;
    let (words, []) = body.as_chunks::<2>() else {
        return Err(Fault::InvalidRawPath);
    };
    let indexed = words.iter().enumerate();
    for (index, word) in indexed {
        if *word == SEPARATOR {
            capture(body, start..index * 2, lease, &mut found)?;
            start = index * 2 + SEPARATOR.len();
        }
    }
    capture(body, start..body.len(), lease, &mut found)?;
    Ok(found)
}

fn capture(
    body: &[u8],
    range: Range<usize>,
    lease: &Lease,
    found: &mut Option<Range<usize>>,
) -> Result<(), Fault> {
    if body.get(range.clone()) == Some(lease.entry.as_slice()) {
        if found.is_some() {
            return Err(Fault::AmbiguousLease);
        }
        *found = Some(range);
    }
    Ok(())
}
