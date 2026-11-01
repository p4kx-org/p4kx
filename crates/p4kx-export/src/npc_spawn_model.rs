// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::{LocalisedText, every_field_required};
use crate::guid_text::CatalogGuid;
use crate::spawn_model::{TagPaths, TagTerm};
use crate::texts::Texts;

/// An item an NPC carries (`SpawnSettingsInventoryItem`).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct SpawnInventoryItem {
    /// The `entityClass` Record, `null` when absent.
    pub(crate) entity_class_guid: Option<CatalogGuid>,
    /// The `quantity`; `null` when absent.
    pub(crate) quantity: Option<i64>,
    /// The `amountInventoriesToFill`; `null` when absent.
    pub(crate) amount_inventories_to_fill: Option<i64>,
    /// The `markAsMissionItem` flag; `null` when absent.
    pub(crate) mark_as_mission_item: Option<bool>,
}

/// The `AutoSpawnSettings` of an NPC option: how many NPCs spawn, where and with what.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AutoSpawnSettings {
    /// The `name`, a developer's label such as `SC_CE -> DA_I (Grenadier x1)`; `null` when empty.
    pub(crate) name: Option<String>,
    /// The `initialActivity` as stored (`Sentry`, `Guard`, `GuardPatrolling`, ...); `null` when
    /// empty.
    pub(crate) initial_activity: Option<String>,
    /// The `excludeShipCrew` flag; `null` when absent.
    pub(crate) exclude_ship_crew: Option<bool>,
    /// The `excludeSpawnGender` as the enum `EExcludeSpawnGender` stores it.
    #[schemars(extend("x-values" = [
        {"value": "Male"},
        {"value": "Female"},
        {"value": "None"}
    ]))]
    pub(crate) exclude_spawn_gender: Option<String>,
    /// The `minGroupSize`; `-1` as stored means no minimum.
    pub(crate) min_group_size: Option<i64>,
    /// The `maxGroupSize`.
    pub(crate) max_group_size: Option<i64>,
    /// The `maxConcurrentSpawns`.
    pub(crate) max_concurrent_spawns: Option<i64>,
    /// The `maxSpawns`.
    pub(crate) max_spawns: Option<i64>,
    /// The `minSpawnDelay`; the Archive states no unit.
    pub(crate) min_spawn_delay: Option<i64>,
    /// The `maxSpawnDelay`; the Archive states no unit.
    pub(crate) max_spawn_delay: Option<i64>,
    /// The `missionAlliedMarker` flag; `null` when absent.
    pub(crate) mission_allied_marker: Option<bool>,
    /// The `isCritical` flag; `null` when absent.
    pub(crate) is_critical: Option<bool>,
    /// The `positiveCharacterTags` and `negativeCharacterTags` the character is searched by.
    pub(crate) character_tags: TagTerm,
    /// The `closetPositiveTags` and `closetNegativeTags`.
    pub(crate) closet_tags: TagTerm,
    /// The `roomPositiveTags` and `roomNegativeTags`.
    pub(crate) room_tags: TagTerm,
    /// The `defendAreaPositiveTags` and `defendAreaNegativeTags`.
    pub(crate) defend_area_tags: TagTerm,
    /// The `scheduleAreaPositiveTags` and `scheduleAreaNegativeTags`.
    pub(crate) schedule_area_tags: TagTerm,
    /// The `entityTags`.
    pub(crate) entity_tags: TagPaths,
    /// The `inventoryItems` in stored order.
    pub(crate) inventory_items: Vec<SpawnInventoryItem>,
    /// The `factionOverride` `Faction` Record, `null` when absent; no kind lists these Records.
    pub(crate) faction_override_guid: Option<CatalogGuid>,
}

/// The name an NPC gets (`MissionPropertyValue_AIName`): drawn at random or given.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AiName<'a> {
    /// The `randomName` flag; `null` when absent.
    pub(crate) random_name: Option<bool>,
    /// The `randomLastName` flag; `null` when absent.
    pub(crate) random_last_name: Option<bool>,
    /// The `randomNickName` flag; `null` when absent.
    pub(crate) random_nick_name: Option<bool>,
    /// The `characterGivenName`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) given_name: LocalisedText<'a>,
    /// The `characterGivenLastName`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) given_last_name: LocalisedText<'a>,
    /// The `characterGivenNickName`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) given_nick_name: LocalisedText<'a>,
    /// The `chanceOfNickName`, 0 to 1; `null` when absent.
    pub(crate) chance_of_nick_name: Option<f32>,
    /// The `characterNameData` Record (`CharacterRandomNameParams`), `null` when absent.
    pub(crate) name_data_guid: Option<CatalogGuid>,
}

impl<'a> AiName<'a> {
    /// Replaces the keys of the three texts with the texts `texts` resolves them to.
    pub(crate) fn resolve(&mut self, texts: &Texts<'a>) {
        for text in [
            &mut self.given_name,
            &mut self.given_last_name,
            &mut self.given_nick_name,
        ] {
            *text = texts.localised(text.key);
        }
    }
}

/// One alternative of an NPC group (`SpawnDescription_NPCOption`): the game spawns one of them.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct NpcSpawnOption<'a> {
    /// The `priority`; `-1` as stored means none.
    pub(crate) priority: Option<i64>,
    /// The `includeLocationAISpawnTags` flag; `null` when absent.
    #[serde(rename = "includeLocationAISpawnTags")]
    pub(crate) include_location_ai_spawn_tags: Option<bool>,
    /// The `weight` of the alternative among its group; `null` when absent.
    pub(crate) weight: Option<f32>,
    /// The `autoSpawnSettings`, `null` when absent.
    pub(crate) auto_spawn_settings: Option<AutoSpawnSettings>,
    /// The `identifierTags`.
    pub(crate) identifier_tags: TagPaths,
    /// The `missionVariableName` of the property the `nameProperty` points to, `null` when absent.
    pub(crate) name_property_variable: Option<String>,
    /// The `embeddedName`, `null` when absent.
    pub(crate) embedded_name: Option<AiName<'a>>,
}

/// A group of NPCs that spawn together (`SpawnDescription_NPC_Group`).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct NpcSpawnGroup<'a> {
    /// The `Name` of the group, `null` when empty.
    pub(crate) name: Option<String>,
    /// The alternatives in stored order.
    pub(crate) options: Vec<NpcSpawnOption<'a>>,
}

/// An NPC spawn description of a contract: what a `MissionPropertyValue_NPCSpawnDescriptions`
/// property states, shared by every contract that states the same.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct NpcSpawnPool<'a> {
    /// The identifier of the content (see the catalog documentation); not a GUID of the Archive.
    pub(crate) guid: CatalogGuid,
    /// The groups in stored order.
    pub(crate) groups: Vec<NpcSpawnGroup<'a>>,
}

impl<'a> NpcSpawnPool<'a> {
    /// Resolves the texts of every embedded name with `texts`.
    pub(crate) fn resolve(&mut self, texts: &Texts<'a>) {
        self.groups
            .iter_mut()
            .flat_map(|group| &mut group.options)
            .filter_map(|option| option.embedded_name.as_mut())
            .for_each(|name| name.resolve(texts));
    }
}
