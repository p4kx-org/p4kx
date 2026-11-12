// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourcedValue, StructValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::cargo_source;
use super::catalog_capacity::ResourceCapacity;
use super::inventory_dimensions::{InventoryVector2, InventoryVector3};
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const ATTACHMENT: &str = "AttachDef";
pub(super) const ITEM_DEFINITION: &str = "SItemDefinition";
const VOLUME: &str = "inventoryOccupancyVolume";
const DIMENSIONS: &str = "inventoryOccupancyDimensions";
const LOCAL_MIN: &str = "inventoryOccupancyLocalBoundsMin";
const LOCAL_MAX: &str = "inventoryOccupancyLocalBoundsMax";
const UI_OVERRIDE: &str = "inventoryOccupancyDimensionsUIOverride";
const FIXED_GRID: &str = "inventoryOccupancyFixedGridDimensionsUIV4";

/// The authored storage occupancy of one item definition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InventoryItemOccupancy<'source> {
    #[serde(rename = "inventoryOccupancyVolume")]
    volume: SourcedValue<ResourceCapacity<'source>>,
    #[serde(rename = "inventoryOccupancyDimensions")]
    dimensions: SourcedValue<InventoryVector3<'source>>,
    #[serde(rename = "inventoryOccupancyLocalBoundsMin")]
    local_bounds_min: SourcedValue<InventoryVector3<'source>>,
    #[serde(rename = "inventoryOccupancyLocalBoundsMax")]
    local_bounds_max: SourcedValue<InventoryVector3<'source>>,
    #[serde(rename = "inventoryOccupancyDimensionsUIOverride")]
    ui_override: SourcedValue<InventoryVector3<'source>>,
    #[serde(rename = "inventoryOccupancyFixedGridDimensionsUIV4")]
    fixed_ui_grid: SourcedValue<InventoryVector2<'source>>,
}

impl<'source> InventoryItemOccupancy<'source> {
    pub(super) fn applies(component: &StructValue<'_>) -> bool {
        let Some(Value::Struct(definition)) = component.field(ATTACHMENT) else {
            return false;
        };
        [
            VOLUME,
            DIMENSIONS,
            LOCAL_MIN,
            LOCAL_MAX,
            UI_OVERRIDE,
            FIXED_GRID,
        ]
        .into_iter()
        .any(|name| definition.field(name).is_some())
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            volume: source.strong_pointer_family_to(
                VOLUME,
                cargo_source::BASE,
                notes,
                ResourceCapacity::read,
            )?,
            dimensions: InventoryVector3::class_field(source, DIMENSIONS, notes)?,
            local_bounds_min: InventoryVector3::class_field(source, LOCAL_MIN, notes)?,
            local_bounds_max: InventoryVector3::class_field(source, LOCAL_MAX, notes)?,
            ui_override: source.strong_pointer_to(
                UI_OVERRIDE,
                "Vec3",
                &["Vec3"],
                notes,
                InventoryVector3::read,
            )?,
            fixed_ui_grid: source.declared_class_to(
                FIXED_GRID,
                "Vec2",
                notes,
                InventoryVector2::read,
            )?,
        })
    }
}
