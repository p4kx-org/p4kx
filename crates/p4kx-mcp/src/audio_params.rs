// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Deserialize;

/// The most bytes accepted in trigger and language arguments.
pub(crate) const MAX_NAME_LEN: usize = 256;

/// The most Entities listed per trigger.
pub(crate) const MAX_ENTITIES: u32 = 100;

/// The Entities listed per trigger when a call gives no number.
pub(crate) const DEFAULT_ENTITIES: u32 = 0;

/// The arguments of `search_audio_triggers`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SearchTriggersParams {
    /// A Channel name such as `LIVE`, or the absolute path of a `.p4k` file; omit it for the
    /// Archive the server was started with.
    pub(crate) archive: Option<String>,
    /// Text in a trigger or Wwise event name, or a glob with `*` and `?` such as `Play_*_Spool`;
    /// ignores case. Omit it to list every trigger.
    pub(crate) search: Option<String>,
    /// The most Entities listed per trigger, 0 by default, at most 100. 0 skips the lookup; any
    /// other value reads the Record of every Entity on each call, which takes seconds on a real
    /// Archive. Only Entities whose own Record names the trigger count.
    pub(crate) entities: Option<u32>,
    /// The `next_cursor` of the previous page; omit it for the first page.
    pub(crate) cursor: Option<String>,
    /// The page size, 100 by default, at most 1000.
    pub(crate) limit: Option<u32>,
}

/// The arguments of `get_entity_sounds`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct EntitySoundsParams {
    /// A Channel name such as `LIVE`, or the absolute path of a `.p4k` file; omit it for the
    /// Archive the server was started with.
    pub(crate) archive: Option<String>,
    /// The Entity: its GUID, its record path or its class name such as `AEGS_Gladius`.
    pub(crate) entity: String,
    /// The `next_cursor` of the previous page; omit it for the first page.
    pub(crate) cursor: Option<String>,
    /// The page size, 100 by default, at most 1000.
    pub(crate) limit: Option<u32>,
}

/// The arguments of `resolve_audio_trigger`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TriggerParams {
    /// A Channel name such as `LIVE`, or the absolute path of a `.p4k` file; omit it for the
    /// Archive the server was started with.
    pub(crate) archive: Option<String>,
    /// The audio trigger as its ATL definition names it, e.g. `Play_QDrive_Spool`; ignores case.
    pub(crate) trigger: String,
    /// The language folder below `Data/Sounds/wwise` to resolve localised sounds in, as the
    /// Archive spells it, e.g. `English(US)`; ignores case. Omit it to use the fallback language.
    pub(crate) lang: Option<String>,
    /// The language folder used where `lang` has no localisation of a sound; `English(US)` by
    /// default, or the first language folder when the Archive has none of that name.
    pub(crate) fallback_lang: Option<String>,
    /// The `next_cursor` of the previous page; omit it for the first page.
    pub(crate) cursor: Option<String>,
    /// The page size, 100 by default, at most 1000.
    pub(crate) limit: Option<u32>,
}

/// The arguments of `list_soundbank_wems`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SoundbankParams {
    /// A Channel name such as `LIVE`, or the absolute path of a `.p4k` file; omit it for the
    /// Archive the server was started with.
    pub(crate) archive: Option<String>,
    /// The Entry path of the soundbank, e.g. `Data/Sounds/wwise/Ship.bnk`, as
    /// `search_audio_triggers` and `resolve_audio_trigger` give it; case and separator kind do
    /// not matter.
    pub(crate) soundbank: String,
    /// The `next_cursor` of the previous page; omit it for the first page.
    pub(crate) cursor: Option<String>,
    /// The page size, 100 by default, at most 1000.
    pub(crate) limit: Option<u32>,
}

/// The arguments of `list_soundbank_objects`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SoundbankObjectsParams {
    /// A Channel name such as `LIVE`, or the absolute path of a `.p4k` file; omit it for the
    /// Archive the server was started with.
    pub(crate) archive: Option<String>,
    /// The Entry path of the soundbank, e.g. `Data/Sounds/wwise/Ship.bnk`, as
    /// `search_audio_triggers` and `resolve_audio_trigger` give it; case and separator kind do
    /// not matter.
    pub(crate) soundbank: String,
    /// Lists only objects of this type, as the `type` of a listed object names it, e.g.
    /// `music-playlist`, `sound`, `event`; omit it for every type.
    #[serde(rename = "type")]
    pub(crate) kind: Option<String>,
    /// Lists only the object with this ID.
    pub(crate) id: Option<u32>,
    /// The `next_cursor` of the previous page; omit it for the first page.
    pub(crate) cursor: Option<String>,
    /// The page size, 100 by default, at most 1000.
    pub(crate) limit: Option<u32>,
}

/// The arguments of `get_wem_info`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct WemParams {
    /// A Channel name such as `LIVE`, or the absolute path of a `.p4k` file; omit it for the
    /// Archive the server was started with.
    pub(crate) archive: Option<String>,
    /// The WEM ID, as `resolve_audio_trigger` and `list_soundbank_wems` give it.
    pub(crate) wem_id: Option<u32>,
    /// The Entry path of a loose WEM, e.g. `Data/Sounds/wwise/12345.wem`; use it instead of
    /// `wem_id` to name one Entry of several languages.
    pub(crate) path: Option<String>,
    /// The `next_cursor` of the previous page of `locations`; omit it for the first page.
    pub(crate) cursor: Option<String>,
    /// The page size of `locations`, 100 by default, at most 1000.
    pub(crate) limit: Option<u32>,
}

/// The arguments of `get_audio_excerpt`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExcerptParams {
    /// A Channel name such as `LIVE`, or the absolute path of a `.p4k` file; omit it for the
    /// Archive the server was started with.
    pub(crate) archive: Option<String>,
    /// The WEM ID, as `resolve_audio_trigger` and `list_soundbank_wems` give it.
    pub(crate) wem_id: Option<u32>,
    /// The Entry path of a loose WEM, e.g. `Data/Sounds/wwise/12345.wem`; use it instead of
    /// `wem_id` to name one Entry of several languages.
    pub(crate) path: Option<String>,
    /// Where the excerpt starts, in seconds from the start of the sound; 0 by default.
    pub(crate) start_seconds: Option<f64>,
    /// The length of the excerpt in seconds, 5 by default, at most 10. The excerpt is also cut
    /// where the WAV would exceed 1 MiB.
    pub(crate) duration_seconds: Option<f64>,
}
