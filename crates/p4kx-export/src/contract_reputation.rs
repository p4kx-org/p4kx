// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::{LocalisedText, every_field_required};
use crate::guid_text::CatalogGuid;

/// A faction: a `FactionReputation` Record under `factions/factionreputation`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct FactionReputation<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `FactionReputation_Wikelo`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The `displayName`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) display_name: LocalisedText<'a>,
    /// The `isNPC` flag.
    pub(crate) is_npc: Option<bool>,
    /// Whether the faction is lawful: the value of the boolean `entityLawful` property of its
    /// `propertiesBB`, `null` when it has none.
    pub(crate) lawful: Option<bool>,
    /// The `SReputationContextUI` Record the faction is shown with (`reputationContextPropertiesUI`),
    /// `null` when it names none.
    pub(crate) context_guid: Option<CatalogGuid>,
    /// The factions it counts as allies, as stored.
    pub(crate) ally_guids: Vec<CatalogGuid>,
    /// The factions it counts as enemies, as stored.
    pub(crate) enemy_guids: Vec<CatalogGuid>,
    /// The scope and standing a member of the faction holds towards a hostile faction
    /// (`hostilityParams`), `null` when the Record has none.
    pub(crate) hostility: Option<RelationStanding>,
    /// The scope and standing a member of the faction holds towards an allied faction
    /// (`alliedParams`), `null` when the Record has none.
    pub(crate) allied: Option<RelationStanding>,
}

/// The scope and standing a faction relation assigns.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RelationStanding {
    /// The `SReputationScopeParams` Record, `null` when the relation names none.
    pub(crate) scope_guid: Option<CatalogGuid>,
    /// The `SReputationStandingParams` Record, `null` when the relation names none.
    pub(crate) standing_guid: Option<CatalogGuid>,
}
