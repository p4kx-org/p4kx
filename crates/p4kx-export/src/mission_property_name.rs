// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::definition_reference::DefinitionReference;
use crate::mission_property_set::MissionPropertyText;
use crate::npc_spawn_model::NpcSpawnName;

/// An investigation name with its established presentation and original source identities.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
pub(crate) struct InvestigationName<'source> {
    /// The name settings and sourced text references.
    #[serde(flatten)]
    #[schemars(with = "NpcSpawnName")]
    pub(crate) name: NpcSpawnName<MissionPropertyText<'source>>,
    /// The raw Locale tokens and complete name-data Reference.
    pub(crate) source: InvestigationNameSource<'source>,
}

/// The original Locale tokens and Record link authored by an investigation name.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
pub(crate) struct InvestigationNameSource<'source> {
    /// The raw `characterGivenName`, retaining empty and placeholder tokens.
    #[serde(rename = "characterGivenName")]
    pub(crate) given_name: Option<&'source str>,
    /// The raw `characterGivenLastName`, retaining empty and placeholder tokens.
    #[serde(rename = "characterGivenLastName")]
    pub(crate) given_last_name: Option<&'source str>,
    /// The raw `characterGivenNickName`, retaining empty and placeholder tokens.
    #[serde(rename = "characterGivenNickName")]
    pub(crate) given_nick_name: Option<&'source str>,
    /// The authored `characterNameData`, including unresolved target identity.
    #[serde(rename = "characterNameData")]
    pub(crate) name_data: Option<DefinitionReference>,
}
