// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::Range;
use std::ptr;

use anyhow::Result;
use p4kx_assembly::audio::{AtlTriggerOrigins, EntityTrigger, EventMatch, SoundbankEventSource};
use p4kx_extraction::{
    FieldOrigin, InputScope, SourceInput, SourceLocation, SourceScope, SourceUse, SourceUseRole,
};

use super::header_origins;
use super::{Input, Output, Surveys};

pub(super) struct TriggerJoins {
    pub(super) definition: FieldOrigin,
    pub(super) events: FieldOrigin,
    pub(super) status: FieldOrigin,
}

pub(super) fn write(
    output: &mut Output,
    input: &Input<'_, '_>,
    surveys: &Surveys,
    trigger: &EntityTrigger<'_>,
    address: &str,
    caller: &FieldOrigin,
) -> Result<TriggerJoins> {
    let Some(definition) = trigger.definition() else {
        let missing = output.union(
            std::iter::once(caller).chain(&surveys.definitions),
            surveys.definitions.is_some(),
        )?;
        return Ok(TriggerJoins {
            definition: missing.clone(),
            events: missing.clone(),
            status: missing,
        });
    };
    let uses = output.uses.clone();
    let source = AtlTriggerOrigins::open(input.archive, input.index, &uses, definition)?;
    let select = output.union([caller, &source.name(SourceUseRole::Selector)?], true)?;
    let definition = output.union([&select, &source.path()?], true)?;
    let topology = output.union([&select, &source.events()?], true)?;
    let mut events = Vec::new();
    let mut alternatives = Vec::new();
    let mut matched = None;
    for (ordinal, event) in trigger.events().enumerate() {
        let event_path = format!("{address}/events/{ordinal}");
        let proof = write_event(output, input, &source, &select, surveys, &event_path, event)?;
        events.push(proof.row);
        if matched.is_none() {
            if event.is_matched() {
                matched = Some(proof.holder);
            } else {
                alternatives.push(proof.holder);
            }
        }
    }
    let events = output.union(std::iter::once(&topology).chain(&events), true)?;
    let status = output.union(
        std::iter::once(&topology)
            .chain(&alternatives)
            .chain(&matched),
        true,
    )?;
    Ok(TriggerJoins {
        definition,
        events,
        status,
    })
}

struct EventProof {
    row: FieldOrigin,
    holder: FieldOrigin,
}

fn write_event(
    output: &mut Output,
    input: &Input<'_, '_>,
    source: &AtlTriggerOrigins<'_>,
    select: &FieldOrigin,
    surveys: &Surveys,
    address: &str,
    matched: EventMatch<'_>,
) -> Result<EventProof> {
    let event = matched.event();
    let name = output.union(
        [select, &source.event_name(event, SourceUseRole::Payload)?],
        true,
    )?;
    let id = output.union(
        [
            select,
            &source.event_name(event, SourceUseRole::Conversion)?,
        ],
        true,
    )?;
    let localised = output.union([select, &source.event_localised(event)?], true)?;
    let (banks, holder) = write_banks(output, input, surveys, address, matched, &id)?;
    let row = output.members(
        address,
        [
            ("name", name),
            ("id", id),
            ("localised", localised),
            ("soundbanks", banks),
        ],
    )?;
    output.put(address, row.clone())?;
    Ok(EventProof { row, holder })
}

fn write_banks(
    output: &mut Output,
    input: &Input<'_, '_>,
    surveys: &Surveys,
    address: &str,
    matched: EventMatch<'_>,
    event: &FieldOrigin,
) -> Result<(FieldOrigin, FieldOrigin)> {
    let mut observed = HashMap::<_, Vec<_>>::new();
    for source in input
        .index
        .event_soundbank_sources(input.archive, matched.event().id())
    {
        observed
            .entry(source.soundbank().entry_position())
            .or_default()
            .push(source);
    }
    let mut banks = Vec::new();
    for (ordinal, bank) in matched.soundbanks().enumerate() {
        let sources = observed
            .get(&bank.entry_position())
            .map_or_default(Vec::as_slice);
        let sources = sources
            .first()
            .filter(|source| ptr::eq(source.soundbank(), bank))
            .map_or(&[][..], |_| sources);
        let bank_path = format!("{address}/soundbanks/{ordinal}");
        banks.push(write_bank(output, &bank_path, sources, event)?);
    }
    let holder = match banks.first() {
        Some(first) => first.clone(),
        None => output.union(
            std::iter::once(event).chain(&surveys.banks),
            surveys.banks.is_some(),
        )?,
    };
    let all = if banks.is_empty() {
        holder.clone()
    } else {
        output.union(std::iter::once(event).chain(&banks), true)?
    };
    Ok((all, holder))
}

fn write_bank(
    output: &mut Output,
    address: &str,
    sources: &[SoundbankEventSource<'_, '_>],
    event: &FieldOrigin,
) -> Result<FieldOrigin> {
    let entry = sources.first().map(SoundbankEventSource::entry);
    let holder = bank_selection(output, sources)?;
    let select = output.union([event, &holder], true)?;
    let path = header_origins::directory(&output.uses, entry, SourceUseRole::Payload)?;
    let language = header_origins::directory(&output.uses, entry, SourceUseRole::Conversion)?;
    let row = output.members(
        address,
        [
            ("path", output.union([&select, &path], true)?),
            ("language", output.union([&select, &language], true)?),
        ],
    )?;
    output.put(address, row.clone())?;
    Ok(row)
}

fn bank_selection(
    output: &Output,
    sources: &[SoundbankEventSource<'_, '_>],
) -> Result<FieldOrigin> {
    let Some(first) = sources.first() else {
        return Ok(output.uses.origin(&[], false));
    };
    let entry = first.entry();
    let scope = output
        .uses
        .observe(SourceInput::ArchiveView(entry), InputScope::Payload)?;
    let mut selected = Vec::new();
    let version = first.version_bytes();
    let complete = version.is_some() && entry.ancestors().all(|ancestor| ancestor.is_some());
    if let Some(bytes) = version {
        selected.push(word(&scope, "bnkBkhd", "version", bytes)?);
    }
    for source in sources {
        selected.push(word(
            &scope,
            "bnkHircObject",
            "typeCode",
            source.kind_bytes(),
        )?);
        selected.push(word(&scope, "bnkHircObject", "id", source.id_bytes())?);
    }
    Ok(output.uses.origin(&selected, complete))
}

fn word(
    scope: &SourceScope,
    namespace: &str,
    field: &str,
    bytes: Range<usize>,
) -> Result<SourceUse> {
    Ok(scope.at(
        SourceLocation::Subresource {
            namespace: namespace.into(),
            indices: vec![bytes.start as u64, bytes.end as u64],
            field: field.into(),
        },
        SourceUseRole::Selector,
    )?)
}
