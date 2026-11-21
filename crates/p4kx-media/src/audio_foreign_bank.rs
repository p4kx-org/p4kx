// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::path::Path;

use p4kx_extraction::bank::{BankHircObject, BankReadError, BankSource};
use p4kx_extraction::{Entry, HircKind, SectionId};

use crate::bank_candidate_index::Observation;
use crate::hirc_object_index::HircObjectIndex;
use crate::origin_input::MediaOriginInput;
use crate::source_selection::EntrySelection;
use crate::{
    DirectoryContext, MediaError, MediaWriter, RequiredInput, Subresource, error, required_input,
};

use super::{Blocker, Header, HeaderRead, Observations, allocation};

pub(super) struct Bank<'directory> {
    pub(super) entry: &'directory Entry,
    pub(super) origin: MediaOriginInput,
    pub(super) header: HeaderRead,
    pub(super) hirc: HircRead,
    pub(super) filtered: bool,
    pub(super) key_required: bool,
}

pub(super) enum HircRead {
    NotRequested,
    Unavailable(Blocker),
    Observed(HircObjectIndex),
}

pub(super) fn collect<'directory>(
    context: &DirectoryContext<'directory, '_, '_>,
    observed: &Observations,
    writer: &mut MediaWriter<'_>,
) -> Result<Vec<Bank<'directory>>, MediaError> {
    let lookup = context.lookup;
    let mut banks = Vec::new();
    for entry in context.entries {
        work(context, writer)?;
        if !crate::audio_provider::is_bank(entry.path()) {
            continue;
        }
        let filtered =
            lookup.selection.classify(lookup.chain, entry.path()) != EntrySelection::Included;
        let key_required = entry.is_encrypted() && !lookup.index.archive.has_key();
        let header = header(context, observed, entry, filtered, key_required);
        let hirc = match header {
            HeaderRead::Unavailable(reason) => HircRead::Unavailable(reason),
            HeaderRead::Known(header) => hirc(context, entry, header, observed, writer)?,
        };
        banks.try_reserve(1).map_err(|cause| allocation(&cause))?;
        banks.push(Bank {
            entry,
            origin: writer.origins.input(lookup.index.archive, entry)?,
            header,
            hirc,
            filtered,
            key_required,
        });
    }
    Ok(banks)
}

fn header(
    context: &DirectoryContext<'_, '_, '_>,
    observed: &Observations,
    entry: &Entry,
    filtered: bool,
    key_required: bool,
) -> HeaderRead {
    if context
        .lookup
        .index
        .archive
        .raw_path(entry.index())
        .is_some()
    {
        return HeaderRead::Unavailable(Blocker::UnprovedPath);
    }
    if key_required {
        return HeaderRead::Unavailable(Blocker::KeyRequired);
    }
    if filtered {
        return HeaderRead::Unavailable(Blocker::Filtered);
    }
    observed
        .headers
        .get(&entry.index())
        .copied()
        .unwrap_or(HeaderRead::Unavailable(Blocker::Unopened))
}

fn hirc(
    context: &DirectoryContext<'_, '_, '_>,
    entry: &Entry,
    header: Header,
    observed: &Observations,
    writer: &mut MediaWriter<'_>,
) -> Result<HircRead, MediaError> {
    let plays = observed.interests.banks.get(&header.value.id);
    let events = &observed.interests.events;
    if plays.is_none() && events.is_empty() {
        return Ok(HircRead::NotRequested);
    }
    if !header.value.is_supported_version() {
        return Ok(HircRead::Unavailable(Blocker::UnsupportedBankVersion));
    }
    let at = Subresource::new("audio-bank-foreign-hirc-replay", vec![entry.index() as u64]);
    let read = required_input::with_retained_reference_input(
        context.lookup,
        entry,
        &at,
        writer,
        |input, writer| read_objects(input, plays, events, writer),
    )?;
    Ok(read.map_or(HircRead::Unavailable(Blocker::Unopened), HircRead::Observed))
}

fn read_objects(
    input: &mut RequiredInput<'_>,
    plays: Option<&HashSet<u32>>,
    events: &HashSet<u32>,
    writer: &MediaWriter<'_>,
) -> Result<HircObjectIndex, MediaError> {
    let size = input.receipt().source.size;
    let mut source = BankSource::open(input, size).map_err(replay_failure)?;
    let mut result = HircObjectIndex::default();
    let mut cursor = source.section_cursor();
    while let Some(section) = source.next_section(&mut cursor) {
        let Some(section) = observed(&mut result, section)? else {
            break;
        };
        if section.id() != SectionId::HIRC {
            continue;
        }
        let Some(objects) = observed(&mut result, source.hirc(&section))? else {
            continue;
        };
        for object in objects {
            writer.checkpoint()?;
            let Some(object) = observed(&mut result, object)? else {
                break;
            };
            if events.contains(&object.id()) || plays.is_some_and(|ids| ids.contains(&object.id()))
            {
                result.insert(Observation {
                    id: object.id(),
                    candidate: object,
                })?;
            }
        }
    }
    Ok(result)
}

fn observed<T>(
    objects: &mut HircObjectIndex,
    value: Result<T, BankReadError>,
) -> Result<Option<T>, MediaError> {
    match value {
        Ok(value) => Ok(Some(value)),
        Err(BankReadError::Format(_)) => {
            objects.mark_incomplete();
            Ok(None)
        }
        Err(cause) => Err(replay_failure(cause)),
    }
}

impl Bank<'_> {
    pub(super) fn matches(&self, object: &BankHircObject, selector: super::Selector) -> bool {
        matches!(selector, super::Selector::Bank { .. })
            || matches!(self.header, HeaderRead::Known(header)
                if object.kind(header.value.version) == HircKind::Event)
    }
}

pub(super) fn work(
    context: &DirectoryContext<'_, '_, '_>,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    context
        .lookup
        .input_state()?
        .budget
        .metadata(0, 1, writer.limits(), writer.checkpoint)
}

fn replay_failure(cause: BankReadError) -> MediaError {
    match cause {
        BankReadError::Io(cause) => error::io(Path::new("retained soundbank"), cause),
        other => MediaError::Contract(format!("retained soundbank replay failed: {other}")),
    }
}
