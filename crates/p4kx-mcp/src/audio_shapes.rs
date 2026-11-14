// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

/// A file of the audio data that was left out because it could not be read.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct IssueShape {
    /// The Entry path.
    pub(crate) path: String,
    /// A stable code for the reason, e.g. `soundbank`.
    pub(crate) kind: &'static str,
    /// The reason.
    pub(crate) message: String,
}

/// A soundbank that holds a Wwise event.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct SoundbankRef {
    /// The Entry path of the soundbank.
    pub(crate) path: String,
    /// The language folder of a localised soundbank.
    pub(crate) language: Option<String>,
}

/// A Wwise event of a trigger with the soundbanks that hold it.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct EventShape {
    /// The event name.
    pub(crate) name: String,
    /// The event ID, the Wwise hash of the name.
    pub(crate) id: u32,
    /// Whether the event is localised.
    pub(crate) localised: bool,
    /// Whether the event name matched the search; absent outside a search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) matched: Option<bool>,
    /// How many soundbanks hold the event.
    pub(crate) soundbank_count: usize,
    /// The first 10 soundbanks whose `HIRC` section holds the event, in path order.
    pub(crate) soundbanks: Vec<SoundbankRef>,
}

/// Why a trigger leads to no soundbank.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct UnmatchedShape {
    /// A stable code: `no_definition`, `no_event` or `no_soundbank`.
    pub(crate) kind: &'static str,
    /// The reason.
    pub(crate) message: String,
}

/// An Entity whose Record names a trigger.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct EntityRef {
    /// The GUID of the Record.
    pub(crate) guid: String,
    /// The record path.
    pub(crate) record_path: String,
    /// The class name of the Entity.
    pub(crate) class_name: String,
}

/// The Entities that name a trigger.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct EntitiesShape {
    /// How many Entities name the trigger.
    pub(crate) count: usize,
    /// The first of them in definition order.
    pub(crate) listed: Vec<EntityRef>,
}

/// An audio trigger a search found.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct TriggerHitShape {
    /// The trigger name as its ATL definition writes it.
    pub(crate) name: String,
    /// The Entry path of the ATL definition.
    pub(crate) definition: Option<String>,
    /// Whether the trigger name matched the search.
    pub(crate) matched_trigger: bool,
    /// How many Wwise events the trigger posts.
    pub(crate) events_total: usize,
    /// Every admitted Wwise event the trigger posts.
    pub(crate) events: Vec<EventShape>,
    /// Why the trigger leads to no soundbank; absent when it does.
    pub(crate) unmatched: Option<UnmatchedShape>,
    /// The Entities that name the trigger; absent when the lookup was skipped or failed.
    pub(crate) entities: Option<EntitiesShape>,
}

/// The audio triggers a search found, one page.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct TriggerSearchResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The search text, if there was one.
    pub(crate) search: Option<String>,
    /// How many triggers matched in all.
    pub(crate) total: usize,
    /// How many triggers the ATL definitions of the Archive define.
    pub(crate) trigger_count: usize,
    /// The triggers on this page in the order of the ATL definitions.
    pub(crate) triggers: Vec<TriggerHitShape>,
    /// Why no Entities are listed, when they were asked for and could not be looked up.
    pub(crate) entities_skipped: Option<String>,
    /// How many Entity Records could not be read and name no trigger.
    pub(crate) unreadable_records: usize,
    /// The first files of the audio data that could not be read, at most 20.
    pub(crate) issues: Vec<IssueShape>,
    /// How many files of the audio data could not be read.
    pub(crate) issues_total: usize,
    /// The cursor of the next page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
}

/// The Record of an Entity.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct EntityShape {
    /// The GUID of the Record.
    pub(crate) guid: String,
    /// The record path.
    pub(crate) record_path: String,
    /// The Record name.
    pub(crate) name: String,
    /// The class name of the Entity.
    pub(crate) class_name: String,
}

/// An audio trigger an Entity names.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct EntityTriggerShape {
    /// The trigger name as the Entity writes it.
    pub(crate) name: String,
    /// Where in the Record the trigger is named first.
    pub(crate) location: String,
    /// How often the Record names the trigger.
    pub(crate) references: u32,
    /// The Entry path of the ATL definition; absent when no definition has the trigger.
    pub(crate) definition: Option<String>,
    /// How many Wwise events the trigger posts.
    pub(crate) events_total: usize,
    /// Every admitted Wwise event the trigger posts.
    pub(crate) events: Vec<EventShape>,
    /// Why the trigger leads to no soundbank; absent when it does.
    pub(crate) unmatched: Option<UnmatchedShape>,
}

/// The audio triggers of an Entity, one page.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct EntitySoundsResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The Entity.
    pub(crate) entity: EntityShape,
    /// How many triggers the Entity names.
    pub(crate) total: usize,
    /// How many of them lead to a soundbank.
    pub(crate) matched: usize,
    /// The triggers on this page in the order the Entity first names them.
    pub(crate) triggers: Vec<EntityTriggerShape>,
    /// The first files of the audio data that could not be read, at most 20.
    pub(crate) issues: Vec<IssueShape>,
    /// How many files of the audio data could not be read.
    pub(crate) issues_total: usize,
    /// The cursor of the next page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
}

/// The languages localised sounds were resolved in.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct LanguageShape {
    /// The language asked for.
    pub(crate) requested: Option<String>,
    /// The language used where the requested one has no localisation.
    pub(crate) fallback: Option<String>,
}

/// A Wwise event a trigger posts.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct TriggerEventShape {
    /// The event name.
    pub(crate) name: String,
    /// The event ID.
    pub(crate) id: u32,
    /// Whether the event is localised.
    pub(crate) localised: bool,
}

/// A resolved trigger.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct TriggerShape {
    /// The trigger name as its ATL definition writes it.
    pub(crate) name: String,
    /// The Entry path of the ATL definition.
    pub(crate) definition: Option<String>,
    /// How many events the trigger posts.
    pub(crate) events_total: usize,
    /// Every admitted event the trigger posts.
    pub(crate) events: Vec<TriggerEventShape>,
}

/// One object of the path from an event to a sound.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct StepShape {
    /// The kind of object, e.g. `event`, `random` or `sound`.
    pub(crate) kind: &'static str,
    /// The object ID.
    pub(crate) id: u32,
}

/// Where the bytes of a WEM are.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct LocationShape {
    /// `soundbank` for a WEM embedded in a soundbank, `entry` for a loose WEM Entry.
    pub(crate) kind: &'static str,
    /// The Entry path of the soundbank or of the loose WEM.
    pub(crate) path: String,
    /// The language folder the location lies in.
    pub(crate) language: Option<String>,
    /// The offset in the `DATA` section of a soundbank.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) offset: Option<u64>,
    /// The length in bytes.
    pub(crate) len: u64,
}

/// One sound a trigger may play.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct VariantShape {
    /// The event the sound is reached from.
    pub(crate) event: String,
    /// The soundbank holding the event.
    pub(crate) soundbank: SoundbankRef,
    /// The ID of the Sound object.
    pub(crate) sound_id: u32,
    /// The WEM ID; absent for plug-in sources.
    pub(crate) wem_id: Option<u32>,
    /// How the source is stored: `embedded`, `streamed`, `prefetch_streamed`, `plugin` or other.
    pub(crate) source: &'static str,
    /// Whether the sound is localised.
    pub(crate) localised: bool,
    /// The language the sound was resolved in.
    pub(crate) language: Option<String>,
    /// Whether the fallback language was used.
    pub(crate) fallback: bool,
    /// Where the WEM is; absent when it is not located.
    pub(crate) location: Option<LocationShape>,
    /// The objects from the event to the sound.
    pub(crate) path: Vec<StepShape>,
}

/// A link of the chain from an event to its sounds that is broken.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct BreakShape {
    /// The event.
    pub(crate) event: String,
    /// The soundbank the break was found in.
    pub(crate) soundbank: Option<SoundbankRef>,
    /// The object the chain breaks at.
    pub(crate) object: Option<StepShape>,
    /// The ID the chain points to, where the reason has one.
    pub(crate) target: Option<u32>,
    /// The objects from the event to the break.
    pub(crate) path: Vec<StepShape>,
    /// A stable code for the reason.
    pub(crate) kind: &'static str,
    /// The reason.
    pub(crate) message: String,
}

/// How many variants a trigger has and how they are located.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
pub(crate) struct CountsShape {
    /// All variants.
    pub(crate) variants: usize,
    /// Variants whose WEM is located.
    pub(crate) located: usize,
    /// Variants with a WEM ID whose WEM is not located.
    pub(crate) unlocated: usize,
    /// Variants of plug-in sources, which have no WEM.
    pub(crate) plugins: usize,
    /// Broken links.
    pub(crate) breaks: usize,
}

/// The sounds of an audio trigger, one page of its variants.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct TriggerSoundsResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The languages localised sounds were resolved in.
    pub(crate) language: LanguageShape,
    /// The trigger.
    pub(crate) trigger: TriggerShape,
    /// How the variants and links count over all pages.
    pub(crate) counts: CountsShape,
    /// The variants on this page.
    pub(crate) variants: Vec<VariantShape>,
    /// The broken links, on the first page only, at most 25;
    /// `counts.breaks` says how many there are.
    pub(crate) breaks: Vec<BreakShape>,
    /// The first files of the audio data that could not be read, at most 20.
    pub(crate) issues: Vec<IssueShape>,
    /// How many files of the audio data could not be read.
    pub(crate) issues_total: usize,
    /// The cursor of the next page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
}
