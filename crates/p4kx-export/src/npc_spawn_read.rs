// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::blueprint::as_struct;
use crate::contract::LocalisedText;
use crate::entity_spawn_model::{
    EntitySpawnGroup, EntitySpawnOption, EntitySpawnPool, EntitySpawnSlot,
};
use crate::mission_layers::{items, key, one};
use crate::npc_spawn_model::{
    AiName, AutoSpawnSettings, NpcSpawnGroup, NpcSpawnOption, NpcSpawnPool, SpawnInventoryItem,
};
use crate::record_fields::{boolean, integer, number, reference, text};
use crate::spawn_model::{TagTerm, content_guid};
use crate::spawn_read::Reader;

impl<'f> Reader<'_, 'f> {
    /// Reads a `MissionPropertyValue_NPCSpawnDescriptions`. The texts of an embedded name hold
    /// their keys only, so the identifier depends on the Archive's content and not on a language;
    /// [`NpcSpawnPool::resolve`] adds the texts.
    ///
    /// # Errors
    ///
    /// Returns the error of the serialisation that makes the identifier of the content.
    pub(crate) fn npcs(
        &self,
        value: &StructValue<'f>,
    ) -> Result<NpcSpawnPool<'f>, serde_json::Error> {
        let groups: Vec<NpcSpawnGroup<'f>> = items(Some(value), "spawnDescriptions")
            .iter()
            .filter_map(as_struct)
            .map(|group| NpcSpawnGroup {
                name: text(Some(group), "Name").map(str::to_owned),
                options: items(Some(group), "options")
                    .iter()
                    .filter_map(as_struct)
                    .map(|option| self.npc_option(option))
                    .collect(),
            })
            .collect();
        Ok(NpcSpawnPool {
            guid: content_guid(&groups)?,
            groups,
        })
    }

    /// Reads a `MissionPropertyValue_EntitySpawnDescriptions`.
    ///
    /// # Errors
    ///
    /// Returns the error of the serialisation that makes the identifier of the content.
    pub(crate) fn entities(
        &self,
        value: &StructValue<'f>,
    ) -> Result<EntitySpawnPool, serde_json::Error> {
        let groups: Vec<EntitySpawnGroup> = items(Some(value), "spawnDescriptions")
            .iter()
            .filter_map(as_struct)
            .map(|group| EntitySpawnGroup {
                name: text(Some(group), "Name").map(str::to_owned),
                slots: items(Some(group), "entities")
                    .iter()
                    .filter_map(as_struct)
                    .map(|slot| EntitySpawnSlot {
                        options: items(Some(slot), "options")
                            .iter()
                            .filter_map(as_struct)
                            .map(|option| self.entity_option(option))
                            .collect(),
                    })
                    .collect(),
            })
            .collect();
        Ok(EntitySpawnPool {
            guid: content_guid(&groups)?,
            groups,
        })
    }

    fn npc_option(&self, option: &StructValue<'f>) -> NpcSpawnOption<'f> {
        let parent = Some(option);
        NpcSpawnOption {
            priority: integer(parent, "priority"),
            include_location_ai_spawn_tags: boolean(parent, "includeLocationAISpawnTags"),
            weight: number(parent, "weight"),
            auto_spawn_settings: one(parent, "autoSpawnSettings")
                .map(|settings| self.auto_spawn(settings)),
            identifier_tags: self.tag_list(parent, "identifierTags"),
            name_property_variable: self.property_variable(parent, "nameProperty"),
            embedded_name: one(parent, "embeddedName").map(ai_name),
        }
    }

    fn auto_spawn(&self, settings: &StructValue<'f>) -> AutoSpawnSettings {
        let parent = Some(settings);
        let owned = |name: &str| text(parent, name).map(str::to_owned);
        let pair = |positive: &str, negative: &str| TagTerm {
            positive_tags: self.tag_list(parent, positive),
            negative_tags: self.tag_list(parent, negative),
        };
        AutoSpawnSettings {
            name: owned("name"),
            initial_activity: owned("initialActivity"),
            exclude_ship_crew: boolean(parent, "excludeShipCrew"),
            exclude_spawn_gender: owned("excludeSpawnGender"),
            min_group_size: integer(parent, "minGroupSize"),
            max_group_size: integer(parent, "maxGroupSize"),
            max_concurrent_spawns: integer(parent, "maxConcurrentSpawns"),
            max_spawns: integer(parent, "maxSpawns"),
            min_spawn_delay: integer(parent, "minSpawnDelay"),
            max_spawn_delay: integer(parent, "maxSpawnDelay"),
            mission_allied_marker: boolean(parent, "missionAlliedMarker"),
            is_critical: boolean(parent, "isCritical"),
            character_tags: pair("positiveCharacterTags", "negativeCharacterTags"),
            closet_tags: pair("closetPositiveTags", "closetNegativeTags"),
            room_tags: pair("roomPositiveTags", "roomNegativeTags"),
            defend_area_tags: pair("defendAreaPositiveTags", "defendAreaNegativeTags"),
            schedule_area_tags: pair("scheduleAreaPositiveTags", "scheduleAreaNegativeTags"),
            entity_tags: self.tag_list(parent, "entityTags"),
            inventory_items: items(parent, "inventoryItems")
                .iter()
                .filter_map(as_struct)
                .map(inventory_item)
                .collect(),
            faction_override_guid: reference(parent, "factionOverride"),
        }
    }

    fn entity_option(&self, option: &StructValue<'f>) -> EntitySpawnOption {
        let parent = Some(option);
        EntitySpawnOption {
            amount: integer(parent, "amount"),
            weight: number(parent, "weight"),
            tags: self.tag_list(parent, "tags"),
            negative_tags: self.tag_list(parent, "negativeTags"),
            markup_tags: self.tag_list(parent, "markupTags"),
            entity_tags: self.tag_list(parent, "entityTags"),
        }
    }
}

fn inventory_item(item: &StructValue<'_>) -> SpawnInventoryItem {
    let parent = Some(item);
    SpawnInventoryItem {
        entity_class_guid: reference(parent, "entityClass"),
        quantity: integer(parent, "quantity"),
        amount_inventories_to_fill: integer(parent, "amountInventoriesToFill"),
        mark_as_mission_item: boolean(parent, "markAsMissionItem"),
    }
}

fn ai_name<'f>(name: &StructValue<'f>) -> AiName<'f> {
    let parent = Some(name);
    let keyed = |field: &str| LocalisedText {
        key: key(parent, field),
        ..LocalisedText::default()
    };
    AiName {
        random_name: boolean(parent, "randomName"),
        random_last_name: boolean(parent, "randomLastName"),
        random_nick_name: boolean(parent, "randomNickName"),
        given_name: keyed("characterGivenName"),
        given_last_name: keyed("characterGivenLastName"),
        given_nick_name: keyed("characterGivenNickName"),
        chance_of_nick_name: number(parent, "chanceOfNickName"),
        name_data_guid: reference(parent, "characterNameData"),
    }
}
