// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::auto_spawn_class::AutoSpawnClass;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

const DATA_TYPE: &str = "AutoSpawnData";
const ASSIGNMENT_TYPE: &str = "OnSpawnAssignment";
const TAG_LIST_TYPE: &str = "TagList";

type RecordLink<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;
type SourceField = (&'static str, SourceValueState);

/// The authored settings shared by direct Records and manual set entries.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct AutoSpawnData<'source> {
    class: SourcedValue<AutoSpawnClass<'source>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    identity: RecordLink<'source>,
    initial_activity: SourcedValue<&'source str>,
    on_spawn_assignment: SourcedValue<SpawnAssignment<'source>>,
    #[serde(flatten)]
    limits: SpawnLimits,
    inventory_items: SourcedValue<Vec<SourcedValue<SpawnInventory<'source>>>>,
    entity_tags: SourcedValue<SpawnTags<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> AutoSpawnData<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        fields.declared_class_to("autoSpawnData", DATA_TYPE, notes, Self::read_body)
    }

    fn read_body(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let class = AutoSpawnClass::read(fields, notes)?;
        let identity = fields.record_link_to("identity", "FactionIdentity", notes)?;
        let initial_activity = fields.borrowed_string("initialActivity", notes)?;
        let on_spawn_assignment = fields.strong_pointer_family_to(
            "onSpawnAssignment",
            ASSIGNMENT_TYPE,
            notes,
            SpawnAssignment::read,
        )?;
        let limits = SpawnLimits::read(fields, notes)?;
        let inventory_items = fields.class_array_to(
            "inventoryItems",
            "SpawnSettingsInventoryItem",
            notes,
            SpawnInventory::read,
        )?;
        let entity_tags = SpawnTags::read_field(fields, "entityTags", notes)?;
        let projected = [
            ("class", class.state()),
            ("identity", identity.state()),
            ("initialActivity", initial_activity.state()),
            ("onSpawnAssignment", on_spawn_assignment.state()),
            ("inventoryItems", inventory_items.state()),
            ("entityTags", entity_tags.state()),
        ];
        let authored_fields =
            fields.authored_members_except(projected.into_iter().chain(limits.projected()))?;
        Ok(Self {
            class,
            identity,
            initial_activity,
            on_spawn_assignment,
            limits,
            inventory_items,
            entity_tags,
            authored_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct SpawnLimits {
    min_group_size: SourcedValue<i32>,
    max_group_size: SourcedValue<i32>,
    max_concurrent_spawns: SourcedValue<i32>,
    max_spawns: SourcedValue<i32>,
    min_spawn_delay: SourcedValue<ResourceFloat32>,
    max_spawn_delay: SourcedValue<ResourceFloat32>,
}

impl SpawnLimits {
    fn read(
        fields: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            min_group_size: fields.int32("minGroupSize", notes)?,
            max_group_size: fields.int32("maxGroupSize", notes)?,
            max_concurrent_spawns: fields.int32("maxConcurrentSpawns", notes)?,
            max_spawns: fields.int32("maxSpawns", notes)?,
            min_spawn_delay: fields.single("minSpawnDelay", notes)?,
            max_spawn_delay: fields.single("maxSpawnDelay", notes)?,
        })
    }

    const fn projected(&self) -> [SourceField; 6] {
        [
            ("minGroupSize", self.min_group_size.state()),
            ("maxGroupSize", self.max_group_size.state()),
            ("maxConcurrentSpawns", self.max_concurrent_spawns.state()),
            ("maxSpawns", self.max_spawns.state()),
            ("minSpawnDelay", self.min_spawn_delay.state()),
            ("maxSpawnDelay", self.max_spawn_delay.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct SpawnAssignment<'source> {
    source_type: SourcedValue<&'source str>,
    kind: AssignmentKind,
    area_positive_tags: SourcedValue<SpawnTags<'source>>,
    area_negative_tags: SourcedValue<SpawnTags<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
enum AssignmentKind {
    Base,
    ScheduleArea,
    DefendArea,
    Authored,
}

impl<'source> SpawnAssignment<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let kind = match fields.source_type()? {
            ASSIGNMENT_TYPE => AssignmentKind::Base,
            "ScheduleAreaAssignment" => AssignmentKind::ScheduleArea,
            "DefendAreaAssignment" => AssignmentKind::DefendArea,
            _ => {
                fields.note_retained_variant(notes)?;
                AssignmentKind::Authored
            }
        };
        let area_positive_tags = SpawnTags::read_field(fields, "areaPositiveTags", notes)?;
        let area_negative_tags = SpawnTags::read_field(fields, "areaNegativeTags", notes)?;
        let authored_fields = fields.authored_members_except([
            ("areaPositiveTags", area_positive_tags.state()),
            ("areaNegativeTags", area_negative_tags.state()),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            kind,
            area_positive_tags,
            area_negative_tags,
            authored_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct SpawnTags<'source> {
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    tags: SourcedValue<Vec<RecordLink<'source>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> SpawnTags<'source> {
    fn read_field(
        fields: Observed<'_, '_, '_, 'source>,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        fields.declared_class_to(name, TAG_LIST_TYPE, notes, Self::read)
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let tags = fields.record_link_array_to("tags", "Tag", notes)?;
        let authored_fields = fields.authored_members_except([("tags", tags.state())])?;
        Ok(Self {
            tags,
            authored_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct SpawnInventory<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    entity_class: RecordLink<'source>,
    quantity: SourcedValue<i32>,
    amount_inventories_to_fill: SourcedValue<i32>,
    mark_as_mission_item: SourcedValue<bool>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> SpawnInventory<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let entity_class = fields.record_link_to("entityClass", "EntityClassDefinition", notes)?;
        let quantity = fields.int32("quantity", notes)?;
        let amount_inventories_to_fill = fields.int32("amountInventoriesToFill", notes)?;
        let mark_as_mission_item = fields.boolean("markAsMissionItem", notes)?;
        let authored_fields = fields.authored_members_except([
            ("entityClass", entity_class.state()),
            ("quantity", quantity.state()),
            (
                "amountInventoriesToFill",
                amount_inventories_to_fill.state(),
            ),
            ("markAsMissionItem", mark_as_mission_item.state()),
        ])?;
        Ok(Self {
            entity_class,
            quantity,
            amount_inventories_to_fill,
            mark_as_mission_item,
            authored_fields,
        })
    }
}
