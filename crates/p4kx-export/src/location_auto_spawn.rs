// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::super::authored_fields::Observed;
use super::super::super::auto_spawn_data::{SpawnInventory, SpawnLimits, SpawnTags};
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

type SourceField = (&'static str, SourceValueState);

/// Authored location NPC spawn settings with source rules and inventory.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct LocationAutoSpawnSettings<'source> {
    name: SourcedValue<&'source str>,
    initial_activity: SourcedValue<&'source str>,
    exclude_ship_crew: SourcedValue<bool>,
    exclude_spawn_gender: SourcedValue<&'source str>,
    #[serde(flatten)]
    limits: SpawnLimits,
    inventory_items: SourcedValue<Vec<SourcedValue<SpawnInventory<'source>>>>,
    #[serde(flatten)]
    tags: LocationSpawnTags<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    faction_override: SourcedValue<Box<SourceRecordReference<'source>>>,
    mission_allied_marker: SourcedValue<bool>,
    is_critical: SourcedValue<bool>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> LocationAutoSpawnSettings<'source> {
    /// Borrows the original spawn-settings name.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<&'source str> {
        &self.name
    }

    /// Borrows the original initial activity.
    #[must_use]
    pub const fn initial_activity(&self) -> &SourcedValue<&'source str> {
        &self.initial_activity
    }

    /// Borrows the authored ship-crew exclusion flag.
    #[must_use]
    pub const fn exclude_ship_crew(&self) -> &SourcedValue<bool> {
        &self.exclude_ship_crew
    }

    /// Borrows the original spawn-gender exclusion label.
    #[must_use]
    pub const fn exclude_spawn_gender(&self) -> &SourcedValue<&'source str> {
        &self.exclude_spawn_gender
    }

    /// Borrows the original group, concurrency and delay limits.
    #[must_use]
    pub const fn limits(&self) -> &SpawnLimits {
        &self.limits
    }

    /// Borrows ordered original inventory-item settings.
    #[must_use]
    pub const fn inventory_items(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<SpawnInventory<'source>>>> {
        &self.inventory_items
    }

    /// Borrows the original spawn, room and area Tag lists.
    #[must_use]
    pub const fn tags(&self) -> &LocationSpawnTags<'source> {
        &self.tags
    }

    /// Borrows the original faction override reference.
    #[must_use]
    pub const fn faction_override(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.faction_override
    }

    /// Borrows the authored mission-allied marker flag.
    #[must_use]
    pub const fn mission_allied_marker(&self) -> &SourcedValue<bool> {
        &self.mission_allied_marker
    }

    /// Borrows the authored critical-spawn flag.
    #[must_use]
    pub const fn is_critical(&self) -> &SourcedValue<bool> {
        &self.is_critical
    }

    /// Borrows additional original fields in source order.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }

    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let name = fields.borrowed_string("name", notes)?;
        let initial_activity = fields.borrowed_string("initialActivity", notes)?;
        let exclude_ship_crew = fields.boolean("excludeShipCrew", notes)?;
        let exclude_spawn_gender =
            fields.borrowed_enum_to("excludeSpawnGender", "EExcludeSpawnGender", notes)?;
        let limits = SpawnLimits::read(fields, notes)?;
        let inventory_items = fields.class_array_to(
            "inventoryItems",
            "SpawnSettingsInventoryItem",
            notes,
            SpawnInventory::read,
        )?;
        let tags = LocationSpawnTags::read(fields, notes)?;
        let faction_override = fields.record_link_to("factionOverride", "Faction", notes)?;
        let mission_allied_marker = fields.boolean("missionAlliedMarker", notes)?;
        let is_critical = fields.boolean("isCritical", notes)?;
        let projected = [
            ("name", name.state()),
            ("initialActivity", initial_activity.state()),
            ("excludeShipCrew", exclude_ship_crew.state()),
            ("excludeSpawnGender", exclude_spawn_gender.state()),
            ("inventoryItems", inventory_items.state()),
            ("factionOverride", faction_override.state()),
            ("missionAlliedMarker", mission_allied_marker.state()),
            ("isCritical", is_critical.state()),
        ];
        let authored_fields = fields.authored_members_except(
            projected
                .into_iter()
                .chain(limits.projected())
                .chain(tags.projected()),
        )?;
        Ok(Self {
            name,
            initial_activity,
            exclude_ship_crew,
            exclude_spawn_gender,
            limits,
            inventory_items,
            tags,
            faction_override,
            mission_allied_marker,
            is_critical,
            authored_fields,
        })
    }
}

/// The original character, closet, room and area Tag-list declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[expect(
    clippy::struct_field_names,
    reason = "The field names mirror the distinct DataForge tag-list declarations."
)]
pub struct LocationSpawnTags<'source> {
    positive_character_tags: SourcedValue<SpawnTags<'source>>,
    negative_character_tags: SourcedValue<SpawnTags<'source>>,
    closet_positive_tags: SourcedValue<SpawnTags<'source>>,
    closet_negative_tags: SourcedValue<SpawnTags<'source>>,
    room_positive_tags: SourcedValue<SpawnTags<'source>>,
    room_negative_tags: SourcedValue<SpawnTags<'source>>,
    defend_area_positive_tags: SourcedValue<SpawnTags<'source>>,
    defend_area_negative_tags: SourcedValue<SpawnTags<'source>>,
    schedule_area_positive_tags: SourcedValue<SpawnTags<'source>>,
    schedule_area_negative_tags: SourcedValue<SpawnTags<'source>>,
    entity_tags: SourcedValue<SpawnTags<'source>>,
}

impl<'source> LocationSpawnTags<'source> {
    /// Borrows the original positive character Tag list.
    #[must_use]
    pub const fn positive_character_tags(&self) -> &SourcedValue<SpawnTags<'source>> {
        &self.positive_character_tags
    }

    /// Borrows the original negative character Tag list.
    #[must_use]
    pub const fn negative_character_tags(&self) -> &SourcedValue<SpawnTags<'source>> {
        &self.negative_character_tags
    }

    /// Borrows the original positive closet Tag list.
    #[must_use]
    pub const fn closet_positive_tags(&self) -> &SourcedValue<SpawnTags<'source>> {
        &self.closet_positive_tags
    }

    /// Borrows the original negative closet Tag list.
    #[must_use]
    pub const fn closet_negative_tags(&self) -> &SourcedValue<SpawnTags<'source>> {
        &self.closet_negative_tags
    }

    /// Borrows the original positive room Tag list.
    #[must_use]
    pub const fn room_positive_tags(&self) -> &SourcedValue<SpawnTags<'source>> {
        &self.room_positive_tags
    }

    /// Borrows the original negative room Tag list.
    #[must_use]
    pub const fn room_negative_tags(&self) -> &SourcedValue<SpawnTags<'source>> {
        &self.room_negative_tags
    }

    /// Borrows the original positive defend-area Tag list.
    #[must_use]
    pub const fn defend_area_positive_tags(&self) -> &SourcedValue<SpawnTags<'source>> {
        &self.defend_area_positive_tags
    }

    /// Borrows the original negative defend-area Tag list.
    #[must_use]
    pub const fn defend_area_negative_tags(&self) -> &SourcedValue<SpawnTags<'source>> {
        &self.defend_area_negative_tags
    }

    /// Borrows the original positive schedule-area Tag list.
    #[must_use]
    pub const fn schedule_area_positive_tags(&self) -> &SourcedValue<SpawnTags<'source>> {
        &self.schedule_area_positive_tags
    }

    /// Borrows the original negative schedule-area Tag list.
    #[must_use]
    pub const fn schedule_area_negative_tags(&self) -> &SourcedValue<SpawnTags<'source>> {
        &self.schedule_area_negative_tags
    }

    /// Borrows the original Entity Tag list.
    #[must_use]
    pub const fn entity_tags(&self) -> &SourcedValue<SpawnTags<'source>> {
        &self.entity_tags
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut read = |name| SpawnTags::read_field(fields, name, notes);
        Ok(Self {
            positive_character_tags: read("positiveCharacterTags")?,
            negative_character_tags: read("negativeCharacterTags")?,
            closet_positive_tags: read("closetPositiveTags")?,
            closet_negative_tags: read("closetNegativeTags")?,
            room_positive_tags: read("roomPositiveTags")?,
            room_negative_tags: read("roomNegativeTags")?,
            defend_area_positive_tags: read("defendAreaPositiveTags")?,
            defend_area_negative_tags: read("defendAreaNegativeTags")?,
            schedule_area_positive_tags: read("scheduleAreaPositiveTags")?,
            schedule_area_negative_tags: read("scheduleAreaNegativeTags")?,
            entity_tags: read("entityTags")?,
        })
    }

    const fn projected(&self) -> [SourceField; 11] {
        [
            (
                "positiveCharacterTags",
                self.positive_character_tags.state(),
            ),
            (
                "negativeCharacterTags",
                self.negative_character_tags.state(),
            ),
            ("closetPositiveTags", self.closet_positive_tags.state()),
            ("closetNegativeTags", self.closet_negative_tags.state()),
            ("roomPositiveTags", self.room_positive_tags.state()),
            ("roomNegativeTags", self.room_negative_tags.state()),
            (
                "defendAreaPositiveTags",
                self.defend_area_positive_tags.state(),
            ),
            (
                "defendAreaNegativeTags",
                self.defend_area_negative_tags.state(),
            ),
            (
                "scheduleAreaPositiveTags",
                self.schedule_area_positive_tags.state(),
            ),
            (
                "scheduleAreaNegativeTags",
                self.schedule_area_negative_tags.state(),
            ),
            ("entityTags", self.entity_tags.state()),
        ]
    }
}
