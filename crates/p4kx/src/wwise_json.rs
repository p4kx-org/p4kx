// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{
    BreakReason, BrokenLink, IndexedSoundbank, LanguageChoice, PathStep, SoundVariant,
    TriggerSounds, WemLocation, WwiseEvent,
};
use p4kx_extraction::{FieldOrigins, SourceInventory};
use serde::Serialize;

use crate::entity_audio::IssueJson;

#[path = "wwise_path_origins.rs"]
mod path_origins;
pub(crate) use path_origins::{
    FileTrigger, capture, capture_extract, capture_files, capture_nested,
};

/// The JSON shape of `p4kx wwise resolve`.
#[derive(Serialize)]
pub(crate) struct ResolveJson<'a> {
    pub(crate) archive: String,
    pub(crate) language: LanguageJson<'a>,
    pub(crate) trigger: TriggerJson<'a>,
    pub(crate) truncated_by_limits: bool,
    pub(crate) variants: Vec<VariantJson<'a>>,
    pub(crate) breaks: Vec<BreakJson<'a>>,
    pub(crate) counts: Counts,
    pub(crate) issues: Vec<IssueJson<'a>>,
    #[serde(rename = "fieldOrigins")]
    pub(crate) field_origins: FieldOrigins,
    pub(crate) provenance: SourceInventory,
}

/// The JSON shape of a resolved trigger.
#[derive(Serialize)]
pub(crate) struct TriggerJson<'a> {
    name: &'a str,
    definition: Option<&'a str>,
    events: Vec<EventJson<'a>>,
}

#[derive(Serialize)]
struct EventJson<'a> {
    name: &'a str,
    id: u32,
    localised: bool,
}

#[derive(Serialize)]
pub(crate) struct VariantJson<'a> {
    event: &'a str,
    soundbank: SoundbankJson<'a>,
    sound_id: u32,
    wem_id: Option<u32>,
    source: &'static str,
    localised: bool,
    language: Option<&'a str>,
    fallback: bool,
    location: Option<LocationJson<'a>>,
    path: Vec<StepJson>,
}

/// The languages localised sounds were resolved in.
#[derive(Serialize)]
pub(crate) struct LanguageJson<'a> {
    pub(crate) requested: Option<&'a str>,
    pub(crate) fallback: Option<&'a str>,
}

impl<'a> LanguageJson<'a> {
    pub(crate) fn of(choice: &'a LanguageChoice) -> Self {
        Self {
            requested: choice.preferred(),
            fallback: choice.fallback(),
        }
    }
}

#[derive(Serialize)]
struct SoundbankJson<'a> {
    path: &'a str,
    language: Option<&'a str>,
}

#[derive(Serialize)]
struct LocationJson<'a> {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    soundbank: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entry: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    offset: Option<u64>,
    len: u64,
}

#[derive(Serialize)]
struct StepJson {
    kind: &'static str,
    id: u32,
}

#[derive(Serialize)]
pub(crate) struct BreakJson<'a> {
    event: &'a str,
    soundbank: Option<SoundbankJson<'a>>,
    object: Option<StepJson>,
    target: Option<u32>,
    path: Vec<StepJson>,
    kind: &'static str,
    message: String,
}

/// How many variants a trigger has, how many of them are located and how many links break.
#[derive(Debug, Clone, Copy, Serialize)]
pub(crate) struct Counts {
    pub(crate) variants: usize,
    pub(crate) located: usize,
    pub(crate) unlocated: usize,
    pub(crate) plugins: usize,
    pub(crate) breaks: usize,
}

impl Counts {
    pub(crate) fn of(sounds: &TriggerSounds<'_>) -> Self {
        let count = |keep: fn(&SoundVariant<'_>) -> bool| {
            sounds
                .variants()
                .iter()
                .filter(|variant| keep(variant))
                .count()
        };
        Self {
            variants: sounds.variants().len(),
            located: count(|variant| variant.location().is_some()),
            unlocated: count(|variant| variant.location().is_none() && variant.wem_id().is_some()),
            plugins: count(|variant| variant.wem_id().is_none()),
            breaks: sounds.breaks().len(),
        }
    }
}

pub(crate) fn trigger<'a>(sounds: &TriggerSounds<'a>) -> TriggerJson<'a> {
    TriggerJson {
        name: sounds.trigger().name(),
        definition: sounds.definition_path(),
        events: sounds.trigger().events().iter().map(event).collect(),
    }
}

fn event(event: &WwiseEvent) -> EventJson<'_> {
    EventJson {
        name: event.name(),
        id: event.id(),
        localised: event.is_localised(),
    }
}

pub(crate) fn variant<'a>(variant: &SoundVariant<'a>) -> VariantJson<'a> {
    VariantJson {
        event: variant.event().name(),
        soundbank: soundbank(variant.soundbank()),
        sound_id: variant.sound_id(),
        wem_id: variant.wem_id(),
        source: variant.source_kind().name(),
        localised: variant.is_localised(),
        language: variant.language(),
        fallback: variant.used_fallback(),
        location: variant.location().and_then(location),
        path: steps(variant.path()),
    }
}

pub(crate) fn broken_link<'a>(link: &BrokenLink<'a>) -> BreakJson<'a> {
    BreakJson {
        event: link.event().name(),
        soundbank: link.soundbank().map(soundbank),
        object: link.object().map(step),
        target: match link.reason() {
            BreakReason::Missing { id }
            | BreakReason::OtherSoundbank { id, .. }
            | BreakReason::Cycle { id } => Some(*id),
            _ => None,
        },
        path: steps(link.path()),
        kind: link.reason().kind(),
        message: link.reason().to_string(),
    }
}

fn soundbank(bank: &IndexedSoundbank) -> SoundbankJson<'_> {
    SoundbankJson {
        path: bank.path(),
        language: bank.language(),
    }
}

fn location<'a>(location: &WemLocation<'a>) -> Option<LocationJson<'a>> {
    match location {
        WemLocation::Soundbank {
            soundbank,
            offset,
            len,
        } => Some(LocationJson {
            kind: "soundbank",
            soundbank: Some(soundbank.path()),
            entry: None,
            offset: Some(*offset),
            len: *len,
        }),
        WemLocation::Entry { path, len, .. } => Some(LocationJson {
            kind: "entry",
            soundbank: None,
            entry: Some(path),
            offset: None,
            len: *len,
        }),
        _ => None,
    }
}

fn steps(path: &[PathStep]) -> Vec<StepJson> {
    path.iter().copied().map(step).collect()
}

const fn step(step: PathStep) -> StepJson {
    StepJson {
        kind: step.kind.name(),
        id: step.id,
    }
}
