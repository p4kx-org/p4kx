// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::contract::LocalisedText;
use crate::entity_spawn_model::{
    EntitySpawnGroup, EntitySpawnOption, EntitySpawnPool, EntitySpawnSlot,
};
use crate::npc_spawn_model::{
    AiName, AutoSpawnSettings, NpcSpawnGroup, NpcSpawnOption, NpcSpawnPool, SpawnInventoryItem,
};
use crate::record_read::ReadError;
use crate::spawn_model::{self, TagTerm};
use crate::spawn_read::{ReadFields, Reader};

impl<'source> Reader<'_, 'source> {
    /// Reads the existing NPC content recipe with untranslated embedded name keys.
    ///
    /// # Errors
    /// Returns original access or content-serialization failures.
    pub(crate) fn npcs(
        &self,
        value: &StructValue<'source>,
    ) -> Result<NpcSpawnPool<'source>, ReadError> {
        let groups = self
            .fields(Some(value))
            .structs("spawnDescriptions")?
            .map(|group| self.npc_group(group?))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(NpcSpawnPool {
            guid: spawn_model::content_guid(&groups)?,
            groups,
        })
    }

    fn npc_group(&self, group: &StructValue<'source>) -> Result<NpcSpawnGroup<'source>, ReadError> {
        let fields = self.fields(Some(group));
        let name = fields.owned("Name")?;
        let options = fields
            .structs("options")?
            .map(|option| self.npc_option(option?))
            .collect::<Result<_, _>>()?;
        Ok(NpcSpawnGroup { name, options })
    }

    /// Reads the existing entity content recipe.
    ///
    /// # Errors
    /// Returns original access or content-serialization failures.
    pub(crate) fn entities(
        &self,
        value: &StructValue<'source>,
    ) -> Result<EntitySpawnPool, ReadError> {
        let groups = self
            .fields(Some(value))
            .structs("spawnDescriptions")?
            .map(|group| self.entity_group(group?))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(EntitySpawnPool {
            guid: spawn_model::content_guid(&groups)?,
            groups,
        })
    }

    fn entity_group(&self, group: &StructValue<'source>) -> Result<EntitySpawnGroup, ReadError> {
        let fields = self.fields(Some(group));
        let name = fields.owned("Name")?;
        let slots = fields
            .structs("entities")?
            .map(|slot| self.entity_slot(slot?))
            .collect::<Result<_, _>>()?;
        Ok(EntitySpawnGroup { name, slots })
    }

    fn entity_slot(&self, slot: &StructValue<'source>) -> Result<EntitySpawnSlot, ReadError> {
        Ok(EntitySpawnSlot {
            options: self
                .fields(Some(slot))
                .structs("options")?
                .map(|option| self.entity_option(option?))
                .collect::<Result<_, _>>()?,
        })
    }

    fn npc_option(
        &self,
        option: &StructValue<'source>,
    ) -> Result<NpcSpawnOption<'source>, ReadError> {
        let fields = self.fields(Some(option));
        Ok(NpcSpawnOption {
            priority: fields.integer("priority")?,
            include_location_ai_spawn_tags: fields.boolean("includeLocationAISpawnTags")?,
            weight: fields.number("weight")?,
            auto_spawn_settings: fields
                .one("autoSpawnSettings")?
                .map(|settings| self.auto_spawn(settings))
                .transpose()?,
            identifier_tags: fields.tag_list("identifierTags")?,
            name_property_variable: fields.variable("nameProperty")?,
            embedded_name: fields
                .one("embeddedName")?
                .map(|name| read_ai_name(&self.fields(Some(name))))
                .transpose()?,
        })
    }

    fn auto_spawn(&self, settings: &StructValue<'source>) -> Result<AutoSpawnSettings, ReadError> {
        let fields = self.fields(Some(settings));
        Ok(AutoSpawnSettings {
            name: fields.owned("name")?,
            initial_activity: fields.owned("initialActivity")?,
            exclude_ship_crew: fields.boolean("excludeShipCrew")?,
            exclude_spawn_gender: fields.owned("excludeSpawnGender")?,
            min_group_size: fields.integer("minGroupSize")?,
            max_group_size: fields.integer("maxGroupSize")?,
            max_concurrent_spawns: fields.integer("maxConcurrentSpawns")?,
            max_spawns: fields.integer("maxSpawns")?,
            min_spawn_delay: fields.integer("minSpawnDelay")?,
            max_spawn_delay: fields.integer("maxSpawnDelay")?,
            mission_allied_marker: fields.boolean("missionAlliedMarker")?,
            is_critical: fields.boolean("isCritical")?,
            character_tags: tag_pair(&fields, "positiveCharacterTags", "negativeCharacterTags")?,
            closet_tags: tag_pair(&fields, "closetPositiveTags", "closetNegativeTags")?,
            room_tags: tag_pair(&fields, "roomPositiveTags", "roomNegativeTags")?,
            defend_area_tags: tag_pair(
                &fields,
                "defendAreaPositiveTags",
                "defendAreaNegativeTags",
            )?,
            schedule_area_tags: tag_pair(
                &fields,
                "scheduleAreaPositiveTags",
                "scheduleAreaNegativeTags",
            )?,
            entity_tags: fields.tag_list("entityTags")?,
            inventory_items: fields
                .structs("inventoryItems")?
                .map(|item| inventory_item(&self.fields(Some(item?))))
                .collect::<Result<_, _>>()?,
            faction_override_guid: fields.reference("factionOverride")?,
        })
    }

    fn entity_option(&self, option: &StructValue<'source>) -> Result<EntitySpawnOption, ReadError> {
        let fields = self.fields(Some(option));
        Ok(EntitySpawnOption {
            amount: fields.integer("amount")?,
            weight: fields.number("weight")?,
            tags: fields.tag_list("tags")?,
            negative_tags: fields.tag_list("negativeTags")?,
            markup_tags: fields.tag_list("markupTags")?,
            entity_tags: fields.tag_list("entityTags")?,
        })
    }
}

fn tag_pair(
    fields: &ReadFields<'_, '_, '_>,
    positive: &str,
    negative: &str,
) -> Result<TagTerm, ReadError> {
    Ok(TagTerm {
        positive_tags: fields.tag_list(positive)?,
        negative_tags: fields.tag_list(negative)?,
    })
}

fn inventory_item(fields: &ReadFields<'_, '_, '_>) -> Result<SpawnInventoryItem, ReadError> {
    Ok(SpawnInventoryItem {
        entity_class_guid: fields.reference("entityClass")?,
        quantity: fields.integer("quantity")?,
        amount_inventories_to_fill: fields.integer("amountInventoriesToFill")?,
        mark_as_mission_item: fields.boolean("markAsMissionItem")?,
    })
}

impl<'source> Reader<'_, 'source> {
    /// Reads AI-name keys through the same admitted field accesses as embedded names.
    pub(crate) fn ai_name(
        &self,
        name: &StructValue<'source>,
    ) -> Result<AiName<'source>, ReadError> {
        read_ai_name(&self.fields(Some(name)))
    }
}

fn read_ai_name<'source>(
    fields: &ReadFields<'_, '_, 'source>,
) -> Result<AiName<'source>, ReadError> {
    let keyed = |field: &str| -> Result<_, ReadError> {
        Ok(LocalisedText {
            key: fields.key(field)?,
            ..LocalisedText::default()
        })
    };
    Ok(AiName {
        random_name: fields.boolean("randomName")?,
        random_last_name: fields.boolean("randomLastName")?,
        random_nick_name: fields.boolean("randomNickName")?,
        given_name: keyed("characterGivenName")?,
        given_last_name: keyed("characterGivenLastName")?,
        given_nick_name: keyed("characterGivenNickName")?,
        chance_of_nick_name: fields.number("chanceOfNickName")?,
        name_data_guid: fields.reference("characterNameData")?,
    })
}
