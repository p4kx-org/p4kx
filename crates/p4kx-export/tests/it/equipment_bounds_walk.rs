// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{
    InventoryBoundsFields, InventoryItemOccupancy, InventoryOwnerBounds,
};
use p4kx_extraction::SourceInventory;
use serde_json::Value;

use super::units;
use crate::canonical_header::wire::Wire;
use crate::commodity_v3_fixture::TestResult;

pub(super) fn row(
    row: &InventoryOwnerBounds<'_>,
    inventory: &SourceInventory,
    expected: &Value,
) -> TestResult {
    let wire = Wire::new(expected, inventory);
    wire.field("ownerGuid")?.guid(&row.owner_guid())?;
    wire.field("ownerRecordIndex")?
        .leaf(row.owner_record_index())?;
    wire.field("componentOrdinal")?
        .leaf(row.component_ordinal())?;
    wire.field("sourceType")?.leaf(row.source_type())?;
    match row.fields() {
        InventoryBoundsFields::ItemOccupancy { attach_def } => {
            wire.field("kind")?.raw("itemOccupancy")?;
            wire.field("attachDef")?.cell(attach_def, occupancy)
        }
        InventoryBoundsFields::VehicleBounds {
            max_bounding_box_size,
        } => {
            wire.field("kind")?.raw("vehicleBounds")?;
            wire.field("maxBoundingBoxSize")?
                .cell(max_bounding_box_size, units::vector3)
        }
        InventoryBoundsFields::ObjectMetadata { has_local_bounds } => {
            wire.field("kind")?.raw("objectMetadata")?;
            wire.field("hasLocalBounds")?.leaf(has_local_bounds)
        }
        _ => Err("a new bounds family needs a typed traversal".into()),
    }
}

fn occupancy(value: &InventoryItemOccupancy<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("inventoryOccupancyVolume")?
        .cell(value.volume(), units::capacity)?;
    wire.field("inventoryOccupancyDimensions")?
        .cell(value.dimensions(), units::vector3)?;
    wire.field("inventoryOccupancyLocalBoundsMin")?
        .cell(value.local_bounds_min(), units::vector3)?;
    wire.field("inventoryOccupancyLocalBoundsMax")?
        .cell(value.local_bounds_max(), units::vector3)?;
    wire.field("inventoryOccupancyDimensionsUIOverride")?
        .cell(value.ui_override(), units::vector3)?;
    wire.field("inventoryOccupancyFixedGridDimensionsUIV4")?
        .cell(value.fixed_ui_grid(), units::vector2)
}
