// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{SourceKind, Unmatched};

/// The Entity whose audio triggers a list shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityInfo {
    /// The class name, the Record name without `EntityClassDefinition.`.
    pub class_name: String,
    /// The GUID of the Record.
    pub guid: String,
    /// The record path.
    pub record_path: String,
}

/// A Wwise event an audio trigger posts, with the number of soundbanks that hold it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventLine {
    /// The event name as the ATL definition writes it.
    pub name: String,
    /// How many soundbanks hold the event.
    pub soundbanks: usize,
    /// Whether the event name matched the search; always `false` outside a search.
    pub matched: bool,
}

/// An audio trigger of the list: an Entity's or a search hit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerRow {
    /// The trigger name.
    pub name: String,
    /// The Wwise events the trigger posts.
    pub events: Vec<EventLine>,
    /// The path of the ATL definition Entry, or `None` when no definition has the trigger.
    pub definition: Option<String>,
    /// Why the trigger leads to no soundbank, or `None` when a soundbank holds one of its events.
    pub unmatched: Option<Unmatched>,
}

/// What the trigger list shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListSource {
    /// The audio triggers an Entity names.
    Entity(EntityInfo),
    /// The triggers and events whose names match the text.
    Search(String),
}

/// The audio triggers shown in the list, in the order of their source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerList {
    /// What the list shows.
    pub source: ListSource,
    /// The rows; for a search the first page of the hits.
    pub rows: Vec<TriggerRow>,
    /// How many triggers matched in all; more than `rows` holds when a search found many.
    pub total: usize,
}

impl TriggerList {
    /// Returns how many rows lead to a soundbank.
    #[must_use]
    pub fn matched_count(&self) -> usize {
        self.rows
            .iter()
            .filter(|row| row.unmatched.is_none())
            .count()
    }
}

/// Whether a sound variant can be played.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playable {
    /// It can be played.
    Yes,
    /// Only the prefetched start of a streamed sound is embedded; the streamed WEM is missing.
    PrefetchOnly,
    /// A source plug-in generates the sound; no WEM holds it.
    Plugin,
    /// An external source supplies the sound at runtime; no file holds it.
    External,
    /// The sound is a MIDI file, which the preview cannot play.
    Midi,
    /// No soundbank or loose WEM Entry holds the WEM.
    Missing,
}

/// Where the bytes of a WEM to play are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Playback {
    Loose(u32),
    Embedded { bank: u32, wem: u32 },
}

/// One sound a trigger may play, with the details of where it comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// The Wwise event the variant is reached from.
    pub event: String,
    /// The file name of the soundbank whose `HIRC` section leads from the event to the sound.
    pub soundbank: String,
    /// The path of that soundbank Entry.
    pub soundbank_path: String,
    /// The WEM ID, or `None` for a source plug-in.
    pub wem_id: Option<u32>,
    /// Where the media comes from.
    pub kind: SourceKind,
    /// The length of the WEM in the soundbank or the Entry that holds it, zero when none does.
    pub size: u64,
    /// The language the sound was resolved in, or `None` when it is not localised.
    pub language: Option<String>,
    /// Whether the language is the fallback, not the one asked for.
    pub fallback: bool,
    pub(crate) playable: Playable,
    pub(crate) playback: Option<Playback>,
}

impl Variant {
    /// Returns whether the variant can be played.
    #[must_use]
    pub const fn playable(&self) -> Playable {
        self.playable
    }

    /// Returns whether the WEM is embedded in a soundbank rather than a loose Entry of its own.
    #[must_use]
    pub const fn is_embedded(&self) -> bool {
        matches!(self.playback, Some(Playback::Embedded { .. }))
    }
}

/// The sounds of the selected trigger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundList {
    /// The trigger name.
    pub trigger: String,
    /// The variants in the order of the trigger's events and soundbanks.
    pub variants: Vec<Variant>,
    /// The places where the way from an event to a WEM breaks.
    pub breaks: Vec<BreakLine>,
}

/// A place where the way from a Wwise event to a WEM breaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakLine {
    /// The Wwise event the way starts at.
    pub event: String,
    /// The file name of the soundbank that was followed, or `None` when none holds the event.
    pub soundbank: Option<String>,
    /// Why the way breaks.
    pub reason: String,
}

/// What the audio index of the Archive holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexInfo {
    /// The audio triggers of the ATL definitions.
    pub triggers: usize,
    /// The soundbanks.
    pub soundbanks: usize,
    /// The language folders, in name order.
    pub languages: Vec<String>,
    /// The language folder the sounds are resolved in unless another is chosen.
    pub default_language: Option<String>,
    /// The Entries that could not be read.
    pub issues: usize,
}
