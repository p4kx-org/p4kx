// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, Entry};
use p4kx_extraction::CancelToken;
use rayon::prelude::*;

use super::banks::BankContents;
use super::read::entry_bytes;
use super::{AtlTrigger, AudioFileError, MAX_ATL_LEN, MAX_SOUNDBANK_LEN, atl, banks};

pub(super) type BankRead = Result<BankContents, AudioFileError>;
pub(super) type DefinitionRead = Result<Vec<AtlTrigger>, AudioFileError>;

/// Reads and parses the ATL definition `entries` in parallel; returns `None` once `cancel` is
/// cancelled.
pub(super) fn read_definitions(
    archive: &Archive,
    entries: &[(usize, &Entry)],
    cancel: &CancelToken,
) -> Option<Vec<DefinitionRead>> {
    entries
        .par_iter()
        .enumerate()
        .map(|(position, &(_, entry))| {
            if cancel.is_cancelled() {
                return None;
            }
            let file = u32::try_from(position).unwrap_or(u32::MAX);
            Some(
                entry_bytes(archive, entry, MAX_ATL_LEN).and_then(|bytes| atl::parse(&bytes, file)),
            )
        })
        .collect()
}

/// Reads and parses the soundbank `entries`, each with its Archive position, in parallel; returns
/// `None` once `cancel` is cancelled.
pub(super) fn read_soundbanks(
    archive: &Archive,
    entries: &[(usize, &Entry)],
    cancel: &CancelToken,
) -> Option<Vec<BankRead>> {
    entries
        .par_iter()
        .map(|&(position, entry)| {
            if cancel.is_cancelled() {
                return None;
            }
            Some(
                entry_bytes(archive, entry, MAX_SOUNDBANK_LEN)
                    .and_then(|bytes| banks::read(entry.path(), position, &bytes)),
            )
        })
        .collect()
}
