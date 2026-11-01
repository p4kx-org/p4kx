// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The names, texts and scripting module of a mission broker entry.

use p4kx_extraction::StructValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::{LocalisedText, every_field_required};
use crate::mission_layers::key;
use crate::record_fields::{boolean, text};
use crate::texts::Texts;

/// The names and game texts of an entry.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Names<'a> {
    /// The `playerFacingDebugName`, `null` when empty.
    pub(crate) debug_name: Option<String>,
    /// The `title`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) title: LocalisedText<'a>,
    /// The `titleHUD`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) hud_title: LocalisedText<'a>,
    /// The `description`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) description: LocalisedText<'a>,
    /// The `missionGiver`: the name the game shows as the giver.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) mission_giver: LocalisedText<'a>,
    /// The `commsChannelName`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) comms_channel_name: LocalisedText<'a>,
    /// The `missionGiverFragmentTags` as stored, `null` when empty.
    pub(crate) giver_fragment_tags: Option<String>,
}

/// The mission scripting module of an entry.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Scripting {
    /// The `missionModule` file of the mission scripting, `null` when empty.
    pub(crate) module: Option<String>,
    /// The `missionModulePerPlayer` flag.
    pub(crate) module_per_player: Option<bool>,
    /// The `moduleHandlesOwnShutdown` flag.
    pub(crate) module_handles_own_shutdown: Option<bool>,
}

/// Reads the names and texts of `root`.
pub(crate) fn names<'f>(texts: &Texts<'f>, root: Option<&StructValue<'f>>) -> Names<'f> {
    let text_of = |name: &str| texts.localised(key(root, name));
    let owned = |name: &str| text(root, name).map(str::to_owned);
    Names {
        debug_name: owned("playerFacingDebugName"),
        title: text_of("title"),
        hud_title: text_of("titleHUD"),
        description: text_of("description"),
        mission_giver: text_of("missionGiver"),
        comms_channel_name: text_of("commsChannelName"),
        giver_fragment_tags: owned("missionGiverFragmentTags"),
    }
}

/// Reads the scripting module of `root`.
pub(crate) fn module(root: Option<&StructValue<'_>>) -> Scripting {
    Scripting {
        module: text(root, "missionModule").map(str::to_owned),
        module_per_player: boolean(root, "missionModulePerPlayer"),
        module_handles_own_shutdown: boolean(root, "moduleHandlesOwnShutdown"),
    }
}
