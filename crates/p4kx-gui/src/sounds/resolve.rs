// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{
    BrokenLink, EntityAudio, EventMatch, SearchPage, SoundVariant, SourceKind, TriggerSounds,
    WemLocation,
};

use super::view::{BreakLine, EventLine, Playable, Playback, SoundList, TriggerRow, Variant};
use crate::EntryIndex;

fn event_line(event: &EventMatch<'_>, matched: bool) -> EventLine {
    EventLine {
        name: event.event().name().to_owned(),
        soundbanks: event.soundbanks().count(),
        matched,
    }
}

/// Returns the rows of the triggers an Entity names.
pub(super) fn entity_rows(resolved: &EntityAudio<'_>) -> Vec<TriggerRow> {
    resolved
        .triggers()
        .iter()
        .map(|trigger| TriggerRow {
            name: trigger.name().to_owned(),
            events: trigger
                .events()
                .map(|event| event_line(&event, false))
                .collect(),
            definition: trigger.definition_path().map(str::to_owned),
            unmatched: trigger.unmatched(),
        })
        .collect()
}

/// Returns the rows of the hits of a search.
pub(super) fn search_rows(page: &SearchPage<'_>) -> Vec<TriggerRow> {
    page.hits()
        .iter()
        .map(|hit| TriggerRow {
            name: hit.trigger().name().to_owned(),
            events: hit
                .events()
                .map(|(event, matched)| event_line(&event, matched))
                .collect(),
            definition: hit.definition_path().map(str::to_owned),
            unmatched: hit.unmatched(),
        })
        .collect()
}

fn variant(index: &EntryIndex, variant: &SoundVariant<'_>) -> Variant {
    let kind = variant.source_kind();
    let entry = index
        .archive()
        .and_then(|archive| variant.wem_location_entry(archive))
        .and_then(|entry| index.id_of_entry(entry.entry()));
    let (playable, playback) = match (variant.wem_id(), kind, variant.location()) {
        (None, SourceKind::External, _) => (Playable::External, None),
        (None, _, _) | (_, SourceKind::Plugin, _) => (Playable::Plugin, None),
        (Some(_), SourceKind::Midi, None) => (Playable::Missing, None),
        (_, SourceKind::Midi, _) => (Playable::Midi, None),
        (Some(_), SourceKind::PrefetchStreamed, Some(WemLocation::Soundbank { .. })) => {
            (Playable::PrefetchOnly, None)
        }
        (Some(wem), _, Some(WemLocation::Soundbank { .. })) => (
            Playable::Yes,
            entry.map(|bank| Playback::Embedded { bank, wem }),
        ),
        (Some(_), _, Some(WemLocation::Entry { .. })) => {
            (Playable::Yes, entry.map(Playback::Loose))
        }
        (Some(_), _, _) => (Playable::Missing, None),
    };
    Variant {
        event: variant.event().name().to_owned(),
        soundbank: variant.soundbank().file_name().to_owned(),
        soundbank_path: variant.soundbank().path().to_owned(),
        wem_id: variant.wem_id(),
        kind,
        size: variant.location().map_or(0, |location| match *location {
            WemLocation::Soundbank { len, .. } | WemLocation::Entry { len, .. } => len,
            _ => 0,
        }),
        language: variant.language().map(str::to_owned),
        fallback: variant.used_fallback(),
        playable,
        playback,
    }
}

fn break_line(link: &BrokenLink<'_>) -> BreakLine {
    BreakLine {
        event: link.event().name().to_owned(),
        soundbank: link.soundbank().map(|bank| bank.file_name().to_owned()),
        reason: link.reason().to_string(),
    }
}

/// Returns the sounds of a trigger as the list shows them.
pub(super) fn sound_list(index: &EntryIndex, sounds: &TriggerSounds<'_>) -> SoundList {
    SoundList {
        trigger: sounds.trigger().name().to_owned(),
        variants: sounds
            .variants()
            .iter()
            .map(|row| variant(index, row))
            .collect(),
        breaks: sounds.breaks().iter().map(break_line).collect(),
    }
}
