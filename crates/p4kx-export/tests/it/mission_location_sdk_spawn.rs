// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    LocationAutoSpawnSettings, LocationSpawnTags, SpawnInventory, SpawnLimits, SpawnTags,
};
use serde_json::{Value, json};

use super::{TestResult, proof::View};

impl View<'_> {
    pub(super) fn spawn(&mut self, value: &LocationAutoSpawnSettings<'_>) -> TestResult<Value> {
        let mut row = json!({
            "name": self.scalar(value.name())?,
            "initialActivity": self.scalar(value.initial_activity())?,
            "excludeShipCrew": self.scalar(value.exclude_ship_crew())?,
            "excludeSpawnGender": self.scalar(value.exclude_spawn_gender())?,
            "inventoryItems": self.sequence(value.inventory_items(), Self::inventory)?,
            "factionOverride": self.cell(value.faction_override(), |view, value| view.reference(value))?,
            "missionAlliedMarker": self.scalar(value.mission_allied_marker())?,
            "isCritical": self.scalar(value.is_critical())?,
            "authoredFields": self.arena(value.authored_fields())?,
        });
        row.as_object_mut()
            .ok_or("spawn object")?
            .extend(self.limits(value.limits())?);
        row.as_object_mut()
            .ok_or("spawn object")?
            .extend(self.spawn_tags(value.tags())?);
        Ok(row)
    }

    fn limits(&mut self, value: &SpawnLimits) -> TestResult<serde_json::Map<String, Value>> {
        let fields = [
            ("minGroupSize", self.scalar(value.min_group_size())?),
            ("maxGroupSize", self.scalar(value.max_group_size())?),
            (
                "maxConcurrentSpawns",
                self.scalar(value.max_concurrent_spawns())?,
            ),
            ("maxSpawns", self.scalar(value.max_spawns())?),
            ("minSpawnDelay", self.float(value.min_spawn_delay())?),
            ("maxSpawnDelay", self.float(value.max_spawn_delay())?),
        ];
        Ok(fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect())
    }

    fn spawn_tags(
        &mut self,
        value: &LocationSpawnTags<'_>,
    ) -> TestResult<serde_json::Map<String, Value>> {
        let fields = [
            ("positiveCharacterTags", value.positive_character_tags()),
            ("negativeCharacterTags", value.negative_character_tags()),
            ("closetPositiveTags", value.closet_positive_tags()),
            ("closetNegativeTags", value.closet_negative_tags()),
            ("roomPositiveTags", value.room_positive_tags()),
            ("roomNegativeTags", value.room_negative_tags()),
            ("defendAreaPositiveTags", value.defend_area_positive_tags()),
            ("defendAreaNegativeTags", value.defend_area_negative_tags()),
            (
                "scheduleAreaPositiveTags",
                value.schedule_area_positive_tags(),
            ),
            (
                "scheduleAreaNegativeTags",
                value.schedule_area_negative_tags(),
            ),
            ("entityTags", value.entity_tags()),
        ];
        fields
            .into_iter()
            .map(|(key, value)| Ok((key.to_owned(), self.cell(value, Self::tags)?)))
            .collect()
    }

    fn tags(&mut self, value: &SpawnTags<'_>) -> TestResult<Value> {
        Ok(json!({
            "tags": self.sequence(value.tags(), |view, value| view.reference(value))?,
            "authoredFields": self.arena(value.authored_fields())?,
        }))
    }

    fn inventory(&mut self, value: &SpawnInventory<'_>) -> TestResult<Value> {
        Ok(json!({
            "entityClass": self.cell(value.entity_class(), |view, value| view.reference(value))?,
            "quantity": self.scalar(value.quantity())?,
            "amountInventoriesToFill": self.scalar(value.amount_inventories_to_fill())?,
            "markAsMissionItem": self.scalar(value.mark_as_mission_item())?,
            "authoredFields": self.arena(value.authored_fields())?,
        }))
    }
}
