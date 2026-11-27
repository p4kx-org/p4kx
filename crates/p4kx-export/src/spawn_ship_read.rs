// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use super::Reader;
use crate::record_read::ReadError;
use crate::spawn_model::{self, ShipSpawnGroup, ShipSpawnOption, ShipSpawnPool, ShipSpawnSlot};

impl<'source> Reader<'_, 'source> {
    pub(super) fn ships(&self, value: &StructValue<'source>) -> Result<ShipSpawnPool, ReadError> {
        let fields = self.fields(Some(value));
        let allowed = fields.boolean("allowedForMissionRestrictedDeliveries")?;
        let groups = fields
            .structs("spawnDescriptions")?
            .map(|group| self.ship_group(group?))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ShipSpawnPool {
            guid: spawn_model::content_guid(&(allowed, &groups))?,
            allowed_for_mission_restricted_deliveries: allowed,
            groups,
        })
    }

    fn ship_group(&self, group: &StructValue<'source>) -> Result<ShipSpawnGroup, ReadError> {
        let fields = self.fields(Some(group));
        let name = fields.owned("Name")?;
        let slots = fields
            .structs("ships")?
            .map(|slot| self.ship_slot(slot?))
            .collect::<Result<_, _>>()?;
        Ok(ShipSpawnGroup { name, slots })
    }

    fn ship_slot(&self, slot: &StructValue<'source>) -> Result<ShipSpawnSlot, ReadError> {
        Ok(ShipSpawnSlot {
            options: self
                .fields(Some(slot))
                .structs("options")?
                .map(|option| self.ship_option(option?))
                .collect::<Result<_, _>>()?,
        })
    }

    fn ship_option(&self, option: &StructValue<'source>) -> Result<ShipSpawnOption, ReadError> {
        let fields = self.fields(Some(option));
        Ok(ShipSpawnOption {
            weight: fields.number("weight")?,
            concurrent_amount: fields.integer("concurrentAmount")?,
            include_location_ai_spawn_tags: fields.boolean("includeLocationAISpawnTags")?,
            crew_inherits_location_faction: fields.boolean("crewInheritsLocationFaction")?,
            tags: fields.tag_list("tags")?,
            negative_tags: fields.tag_list("negativeTags")?,
            markup_tags: fields.tag_list("markupTags")?,
            entity_tags: fields.tag_list("entityTags")?,
            initial_damage_guid: fields.reference("initialDamageSettings")?,
            name_property_variable: fields.variable("nameProperty")?,
        })
    }
}
