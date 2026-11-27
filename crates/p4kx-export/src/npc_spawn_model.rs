// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::{LocalisedText, every_field_required};
use crate::guid_text::CatalogGuid;
use crate::spawn_model::{TagPaths, TagTerm};

#[path = "npc_spawn_names.rs"]
mod names;
pub use names::NpcSpawnName;
pub(crate) use names::PendingNpcName;

pub(crate) type DraftNpcPool<'source> = NpcSpawnPool<PendingNpcName<'source>>;
/// An NPC spawn pool whose original name keys have been published as protected text cells.
pub type PublishedNpcPool = NpcSpawnPool<NpcSpawnName>;

/// An item an NPC carries (`SpawnSettingsInventoryItem`).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct SpawnInventoryItem {
    /// The `entityClass` Record, `null` when absent.
    pub entity_class_guid: Option<CatalogGuid>,
    /// The `quantity`; `null` when absent.
    pub quantity: Option<i64>,
    /// The `amountInventoriesToFill`; `null` when absent.
    pub amount_inventories_to_fill: Option<i64>,
    /// The `markAsMissionItem` flag; `null` when absent.
    pub mark_as_mission_item: Option<bool>,
}

/// The `AutoSpawnSettings` of an NPC option: how many NPCs spawn, where and with what.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct AutoSpawnSettings {
    /// The `name`, a developer's label such as `SC_CE -> DA_I (Grenadier x1)`; `null` when empty.
    pub name: Option<String>,
    /// The `initialActivity` as stored (`Sentry`, `Guard`, `GuardPatrolling`, ...); `null` when
    /// empty.
    pub initial_activity: Option<String>,
    /// The `excludeShipCrew` flag; `null` when absent.
    pub exclude_ship_crew: Option<bool>,
    /// The `excludeSpawnGender` as the enum `EExcludeSpawnGender` stores it.
    #[schemars(extend("x-values" = [
        {"value": "Male"},
        {"value": "Female"},
        {"value": "None"}
    ]))]
    pub exclude_spawn_gender: Option<String>,
    /// The `minGroupSize`; `-1` as stored means no minimum.
    pub min_group_size: Option<i64>,
    /// The `maxGroupSize`.
    pub max_group_size: Option<i64>,
    /// The `maxConcurrentSpawns`.
    pub max_concurrent_spawns: Option<i64>,
    /// The `maxSpawns`.
    pub max_spawns: Option<i64>,
    /// The `minSpawnDelay`; the Archive states no unit.
    pub min_spawn_delay: Option<i64>,
    /// The `maxSpawnDelay`; the Archive states no unit.
    pub max_spawn_delay: Option<i64>,
    /// The `missionAlliedMarker` flag; `null` when absent.
    pub mission_allied_marker: Option<bool>,
    /// The `isCritical` flag; `null` when absent.
    pub is_critical: Option<bool>,
    /// The `positiveCharacterTags` and `negativeCharacterTags` the character is searched by.
    pub character_tags: TagTerm,
    /// The `closetPositiveTags` and `closetNegativeTags`.
    pub closet_tags: TagTerm,
    /// The `roomPositiveTags` and `roomNegativeTags`.
    pub room_tags: TagTerm,
    /// The `defendAreaPositiveTags` and `defendAreaNegativeTags`.
    pub defend_area_tags: TagTerm,
    /// The `scheduleAreaPositiveTags` and `scheduleAreaNegativeTags`.
    pub schedule_area_tags: TagTerm,
    /// The `entityTags`.
    pub entity_tags: TagPaths,
    /// The `inventoryItems` in stored order.
    pub inventory_items: Vec<SpawnInventoryItem>,
    /// The `factionOverride` `Faction` Record, `null` when absent; no kind lists these Records.
    pub faction_override_guid: Option<CatalogGuid>,
}

/// The name settings and authored keys used to identify an NPC pool.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiName<'a> {
    /// The `randomName` flag; `null` when absent.
    pub(crate) random_name: Option<bool>,
    /// The `randomLastName` flag; `null` when absent.
    pub(crate) random_last_name: Option<bool>,
    /// The `randomNickName` flag; `null` when absent.
    pub(crate) random_nick_name: Option<bool>,
    /// The `characterGivenName`.
    pub(crate) given_name: LocalisedText<'a>,
    /// The `characterGivenLastName`.
    pub(crate) given_last_name: LocalisedText<'a>,
    /// The `characterGivenNickName`.
    pub(crate) given_nick_name: LocalisedText<'a>,
    /// The `chanceOfNickName`, 0 to 1; `null` when absent.
    pub(crate) chance_of_nick_name: Option<f32>,
    /// The `characterNameData` Record (`CharacterRandomNameParams`), `null` when absent.
    pub(crate) name_data_guid: Option<CatalogGuid>,
}

/// One alternative of an NPC group (`SpawnDescription_NPCOption`): the game spawns one of them.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "NpcSpawnOption", transform = every_field_required)]
#[non_exhaustive]
pub struct NpcSpawnOption<Name> {
    /// The `priority`; `-1` as stored means none.
    pub priority: Option<i64>,
    /// The `includeLocationAISpawnTags` flag; `null` when absent.
    #[serde(rename = "includeLocationAISpawnTags")]
    pub include_location_ai_spawn_tags: Option<bool>,
    /// The `weight` of the alternative among its group; `null` when absent.
    pub weight: Option<f32>,
    /// The `autoSpawnSettings`, `null` when absent.
    pub auto_spawn_settings: Option<AutoSpawnSettings>,
    /// The `identifierTags`.
    pub identifier_tags: TagPaths,
    /// The `missionVariableName` of the property the `nameProperty` points to, `null` when absent.
    pub name_property_variable: Option<String>,
    /// The `embeddedName`, `null` when absent.
    pub embedded_name: Option<Name>,
}

/// A group of NPCs that spawn together (`SpawnDescription_NPC_Group`).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "NpcSpawnGroup", transform = every_field_required)]
#[non_exhaustive]
pub struct NpcSpawnGroup<Name> {
    /// The `Name` of the group, `null` when empty.
    pub name: Option<String>,
    /// The alternatives in stored order.
    pub options: Vec<NpcSpawnOption<Name>>,
}

/// An NPC spawn description of a contract: what a `MissionPropertyValue_NPCSpawnDescriptions`
/// property states, shared by every contract that states the same.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "NpcSpawnPool", transform = every_field_required)]
#[non_exhaustive]
pub struct NpcSpawnPool<Name> {
    /// The identifier of the content (see the catalog documentation); not a GUID of the Archive.
    pub guid: CatalogGuid,
    /// The groups in stored order.
    pub groups: Vec<NpcSpawnGroup<Name>>,
}
