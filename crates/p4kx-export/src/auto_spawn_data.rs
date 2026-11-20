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
#[non_exhaustive]
pub struct AutoSpawnData<'source> {
    /// The authored class selector.
    pub class: SourcedValue<AutoSpawnClass<'source>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    /// The original identity reference.
    pub identity: RecordLink<'source>,
    /// The authored initial activity identifier.
    pub initial_activity: SourcedValue<&'source str>,
    /// The authored spawn assignment.
    pub on_spawn_assignment: SourcedValue<SpawnAssignment<'source>>,
    #[serde(flatten)]
    /// The original group, concurrency and delay limits.
    pub limits: SpawnLimits,
    /// The authored inventory slots in source order.
    pub inventory_items: SourcedValue<Vec<SourcedValue<SpawnInventory<'source>>>>,
    /// The authored entity tags.
    pub entity_tags: SourcedValue<SpawnTags<'source>>,
    /// Additional original fields.
    pub authored_fields: SourceArenaOutput<'source>,
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

/// Original group, concurrency and delay limits.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct SpawnLimits {
    min_group_size: SourcedValue<i32>,
    max_group_size: SourcedValue<i32>,
    max_concurrent_spawns: SourcedValue<i32>,
    max_spawns: SourcedValue<i32>,
    min_spawn_delay: SourcedValue<ResourceFloat32>,
    max_spawn_delay: SourcedValue<ResourceFloat32>,
}

impl SpawnLimits {
    /// Borrows the original minimum group size.
    #[must_use]
    pub const fn min_group_size(&self) -> &SourcedValue<i32> {
        &self.min_group_size
    }

    /// Borrows the original maximum group size.
    #[must_use]
    pub const fn max_group_size(&self) -> &SourcedValue<i32> {
        &self.max_group_size
    }

    /// Borrows the original concurrent-spawn maximum.
    #[must_use]
    pub const fn max_concurrent_spawns(&self) -> &SourcedValue<i32> {
        &self.max_concurrent_spawns
    }

    /// Borrows the original total spawn maximum.
    #[must_use]
    pub const fn max_spawns(&self) -> &SourcedValue<i32> {
        &self.max_spawns
    }

    /// Borrows the original minimum spawn delay.
    #[must_use]
    pub const fn min_spawn_delay(&self) -> &SourcedValue<ResourceFloat32> {
        &self.min_spawn_delay
    }

    /// Borrows the original maximum spawn delay.
    #[must_use]
    pub const fn max_spawn_delay(&self) -> &SourcedValue<ResourceFloat32> {
        &self.max_spawn_delay
    }

    pub(super) fn read(
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

    pub(super) const fn projected(&self) -> [SourceField; 6] {
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

/// An authored instruction about where or how to place the spawn.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct SpawnAssignment<'source> {
    /// The declared assignment type.
    pub source_type: SourcedValue<&'source str>,
    /// The typed interpretation of the assignment.
    pub kind: AssignmentKind,
    /// The original positive area tag terms.
    pub area_positive_tags: SourcedValue<SpawnTags<'source>>,
    /// The original negative area tag terms.
    pub area_negative_tags: SourcedValue<SpawnTags<'source>>,
    /// Additional original fields.
    pub authored_fields: SourceArenaOutput<'source>,
}

/// The typed interpretation of an authored spawn assignment.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum AssignmentKind {
    /// The original base assignment.
    Base,
    /// An original schedule-area assignment.
    ScheduleArea,
    /// An original defend-area assignment.
    DefendArea,
    /// An additional original assignment type retained in authored fields.
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

/// One original Tag-list declaration and its additional fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct SpawnTags<'source> {
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    tags: SourcedValue<Vec<RecordLink<'source>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> SpawnTags<'source> {
    /// Borrows the ordered original Tag-reference slots.
    #[must_use]
    pub const fn tags(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>> {
        &self.tags
    }

    /// Borrows additional original fields in source order.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }

    pub(super) fn read_field(
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

/// One original spawn inventory-item declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct SpawnInventory<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    entity_class: RecordLink<'source>,
    quantity: SourcedValue<i32>,
    amount_inventories_to_fill: SourcedValue<i32>,
    mark_as_mission_item: SourcedValue<bool>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> SpawnInventory<'source> {
    /// Borrows the original inventory Entity-class reference.
    #[must_use]
    pub const fn entity_class(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.entity_class
    }

    /// Borrows the original item quantity.
    #[must_use]
    pub const fn quantity(&self) -> &SourcedValue<i32> {
        &self.quantity
    }

    /// Borrows the original number of inventories to fill.
    #[must_use]
    pub const fn amount_inventories_to_fill(&self) -> &SourcedValue<i32> {
        &self.amount_inventories_to_fill
    }

    /// Borrows the authored mission-item marker flag.
    #[must_use]
    pub const fn mark_as_mission_item(&self) -> &SourcedValue<bool> {
        &self.mark_as_mission_item
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
