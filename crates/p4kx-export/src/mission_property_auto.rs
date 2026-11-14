// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::definition_reference::DefinitionReference;
use crate::mission_property_source::SourceTagList;

/// The source configuration of `MissionPropertyValue_AutoSpawns`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
pub(crate) struct AutoSpawns {
    /// The declared `AutoSpawns` Record without following it.
    pub(crate) auto_spawns: Option<DefinitionReference>,
    /// The authored closet-search configuration.
    pub(crate) search_options: Option<SpawnClosetSearchOptions>,
}

/// The concrete authored closet-search variant.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum SpawnClosetSearchOptions {
    /// The fieldless `SpawnClosetSearchOptionsBase`.
    Base,
    /// `SpawnClosetSearchOptions_EntityDeclaration`.
    #[serde(rename_all = "camelCase")]
    EntityDeclaration {
        /// The declared location entity without following the Record.
        entity_declaration: Option<DefinitionReference>,
    },
    /// `SpawnClosetSearchOptions_Tags`.
    #[serde(rename_all = "camelCase")]
    Tags {
        /// The authored positive tag list.
        closet_positive_tags: Option<SourceTagList>,
        /// The authored negative tag list.
        closet_negative_tags: Option<SourceTagList>,
    },
}
