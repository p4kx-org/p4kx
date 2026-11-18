// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective canonical original equipment families without resolved Ship or Loadout work.

#[path = "equipment_attachment_query.rs"]
mod attachments;
#[path = "equipment_bounds_query.rs"]
mod bounds;
#[path = "equipment_item_query.rs"]
mod definitions;
#[path = "equipment_query_read.rs"]
mod read;
#[path = "equipment_query_rows.rs"]
mod rows;
#[path = "equipment_query_snapshot.rs"]
mod snapshot;

pub use crate::catalog_v3::authored_fields::authored_manufacturer::AuthoredManufacturer;
pub use crate::catalog_v3::catalog_capacity::ResourceCapacity;
pub use crate::catalog_v3::catalog_item_alignment::{ItemAttachment, ItemAttachmentComponent};
pub use crate::catalog_v3::catalog_item_components::item_definition::{
    ItemAttachmentPresentation, ItemDefinition, ItemDefinitionComponent,
    ItemDefinitionComponentKind, ItemLocalization, ItemPhysics, ItemPurchasable,
    WeaponAttachmentMotion,
};
pub use crate::catalog_v3::catalog_unit_source::UnitSource;
pub use crate::catalog_v3::inventory_dimensions::{InventoryVector2, InventoryVector3};
pub use crate::catalog_v3::inventory_item_occupancy::InventoryItemOccupancy;
pub use crate::catalog_v3::inventory_owner_bounds::{
    BoundsFields as InventoryBoundsFields, InventoryOwnerBounds,
};
pub use attachments::{with_attachment_constraints, with_attachment_constraints_from};
pub use bounds::{with_item_bounds, with_item_bounds_from};
pub use definitions::{with_item_definitions, with_item_definitions_from};
pub use snapshot::{
    AttachmentConstraintSnapshot, EquipmentSnapshot, ItemBoundsSnapshot, ItemDefinitionSnapshot,
};
