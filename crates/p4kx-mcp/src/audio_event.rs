// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{
    AtlTrigger, AudioIndex, EntityTrigger, EventMatch, IndexedSoundbank, TriggerSounds, WwiseEvent,
};

use crate::audio_convert as convert;
use crate::audio_shapes::{EntityTriggerShape, EventShape, TriggerEventShape, TriggerShape};
use crate::budget::Budget;
use crate::error::ToolError;

/// The most events listed per trigger; the response states the original total.
pub(super) const MAX_EVENTS: usize = 25;
/// The most listed soundbanks per event; the response states the original total.
const MAX_BANKS: usize = 10;

fn event<'index>(
    event: &WwiseEvent,
    banks: impl Iterator<Item = &'index IndexedSoundbank>,
    matched: Option<bool>,
    text: &mut Budget,
) -> Result<EventShape, ToolError> {
    let name = text.text(event.name())?;
    let mut soundbank_count = 0;
    let mut soundbanks = Vec::new();
    for bank in banks {
        soundbank_count += 1;
        if soundbanks.len() < MAX_BANKS {
            soundbanks.push(convert::soundbank(bank, text)?);
        }
    }
    Ok(EventShape {
        name,
        id: event.id(),
        localised: event.is_localised(),
        matched,
        soundbank_count,
        soundbanks,
    })
}

fn events<'index>(
    all: impl Iterator<Item = (EventMatch<'index>, Option<bool>)>,
    text: &mut Budget,
) -> Result<(Vec<EventShape>, usize), ToolError> {
    let mut total = 0;
    let mut listed = Vec::new();
    for (found, matched) in all {
        total += 1;
        if listed.len() < MAX_EVENTS {
            listed.push(event(found.event(), found.soundbanks(), matched, text)?);
        }
    }
    Ok((listed, total))
}

pub(super) fn hit_events(
    index: &AudioIndex,
    trigger: &AtlTrigger,
    matched: &[bool],
    text: &mut Budget,
) -> Result<(Vec<EventShape>, usize), ToolError> {
    if matched.len() != trigger.events().len().min(MAX_EVENTS) {
        return Err(ToolError::Refused(
            "selected audio event decisions do not match the index".to_owned(),
        ));
    }
    let mut listed = Vec::new();
    for (source, matched) in trigger.events().iter().take(MAX_EVENTS).zip(matched) {
        listed.push(event(
            source,
            index.event_soundbanks(source.id()),
            Some(*matched),
            text,
        )?);
    }
    Ok((listed, trigger.events().len()))
}

pub(super) fn entity_trigger(trigger: &EntityTrigger<'_>) -> Result<EntityTriggerShape, ToolError> {
    let mut text = Budget::new();
    let name = text.text(trigger.name())?;
    let location = text.text(trigger.reference().location())?;
    let (events, events_total) = events(trigger.events().map(|event| (event, None)), &mut text)?;
    Ok(EntityTriggerShape {
        name,
        location,
        references: trigger.reference().count(),
        definition: trigger
            .definition_path()
            .map(|value| text.text(value))
            .transpose()?,
        events_total,
        events,
        unmatched: trigger
            .unmatched()
            .map(|value| convert::unmatched(value, &mut text))
            .transpose()?,
    })
}

pub(super) fn trigger(
    sounds: &TriggerSounds<'_>,
    text: &mut Budget,
) -> Result<TriggerShape, ToolError> {
    Ok(TriggerShape {
        name: text.text(sounds.trigger().name())?,
        definition: sounds
            .definition_path()
            .map(|value| text.text(value))
            .transpose()?,
        events_total: sounds.trigger().events().len(),
        events: sounds
            .trigger()
            .events()
            .iter()
            .take(MAX_EVENTS)
            .map(|event| {
                Ok(TriggerEventShape {
                    name: text.text(event.name())?,
                    id: event.id(),
                    localised: event.is_localised(),
                })
            })
            .collect::<Result<_, ToolError>>()?,
    })
}
