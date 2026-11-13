// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::TryReserveError;
use std::io;

use super::DirectoryLookup;
use crate::origin_lookup::ObservedInputLookup;
use crate::{
    CandidateInventory, EntryInput, InputLookup, MediaError, MediaKind, MediaWriter,
    OriginalIdentifier, ReferenceResolution,
};

const BATCH_LENGTH: usize = 4096;

pub(crate) struct AnimationBanks {
    by_crc: HashMap<u32, Vec<usize>>,
    inventory: CandidateInventory,
}

impl AnimationBanks {
    pub(crate) fn read(
        directory: &DirectoryLookup<'_>,
        writer: &MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let inventory = directory.scan_inventory(writer)?;
        let budget = &directory.input_state()?.budget;
        let entries = directory.index.archive.entries();
        let mut by_crc: HashMap<u32, Vec<usize>> = HashMap::new();
        for batch in entries.chunks(BATCH_LENGTH) {
            budget.metadata(0, batch.len() as u64, writer.limits(), writer.checkpoint)?;
            for entry in batch {
                let Some(crc) = bank_crc(entry.path()) else {
                    continue;
                };
                if !by_crc.contains_key(&crc) {
                    by_crc
                        .try_reserve(1)
                        .map_err(|cause| allocation(writer, cause))?;
                }
                let bucket = by_crc.entry(crc).or_default();
                bucket
                    .try_reserve(1)
                    .map_err(|cause| allocation(writer, cause))?;
                bucket.push(entry.index());
            }
        }
        Ok(Self { by_crc, inventory })
    }

    pub(crate) fn lookup<'a>(
        &self,
        directory: &DirectoryLookup<'a>,
        crc: u32,
        original: OriginalIdentifier,
        writer: &mut MediaWriter<'_>,
    ) -> Result<ObservedInputLookup<'a>, MediaError> {
        let budget = &directory.input_state()?.budget;
        budget.metadata(0, 1, writer.limits(), writer.checkpoint)?;
        let positions = self.by_crc.get(&crc).map_or(&[][..], Vec::as_slice);
        let requested = writer.has_kind(MediaKind::Animation);
        let observer = writer.origins.observer()?;
        let archive = directory.index.archive;
        let candidates = positions.iter().map(|position| {
            let entry = archive.entries().get(*position).ok_or_else(missing_entry)?;
            Ok((
                directory.candidate(entry, Some(MediaKind::Animation), requested),
                observer.entry(archive, entry)?,
            ))
        });
        let (reference, evidence) =
            writer.try_resolve_reference_observed(original, candidates, self.inventory)?;
        let input = resolved_input(directory, positions, reference.resolution());
        let inventory = crate::origin_lookup::directory(
            if directory.chain.is_empty() {
                writer.origins.root.as_ref()
            } else {
                writer.origins.directory()
            },
            writer,
        )?;
        Ok(ObservedInputLookup {
            lookup: InputLookup { reference, input },
            evidence,
            inventory,
        })
    }
}

fn resolved_input<'a>(
    directory: &DirectoryLookup<'a>,
    positions: &[usize],
    resolution: &ReferenceResolution,
) -> Option<EntryInput<'a>> {
    let [position] = positions else { return None };
    matches!(resolution, ReferenceResolution::Resolved { .. }).then_some(EntryInput {
        root: directory.root,
        chain: directory.chain,
        position: *position,
    })
}

fn bank_crc(path: &str) -> Option<u32> {
    let (root, child) = path.split_once(['/', '\\'])?;
    if !root.eq_ignore_ascii_case("data") {
        return None;
    }
    let (_, extension) = child.rsplit_once('.')?;
    if !extension.eq_ignore_ascii_case("dba") {
        return None;
    }
    let mut hash = crc32fast::Hasher::new();
    for byte in child.bytes() {
        let normalized = if byte == b'\\' {
            b'/'
        } else {
            byte.to_ascii_lowercase()
        };
        hash.update(&[normalized]);
    }
    Some(hash.finalize())
}

fn missing_entry() -> MediaError {
    MediaError::Contract("indexed DBA has no physical archive Entry".into())
}

fn allocation(writer: &MediaWriter<'_>, cause: TryReserveError) -> MediaError {
    MediaError::Io {
        path: writer.root.clone(),
        source: io::Error::new(io::ErrorKind::OutOfMemory, cause),
    }
}
