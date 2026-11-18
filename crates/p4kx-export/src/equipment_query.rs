// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective queries over original equipment, resolved Ships and Loadouts.

#[path = "equipment_dependency_coverage.rs"]
mod dependency_coverage;
#[path = "equipment_loadout_query.rs"]
mod loadout_query;

pub use crate::catalog_v3::catalog_item_alignment::{
    AttachmentDefinitionLink, ComponentOwner, PortDefinitionLink,
};
pub use crate::loadout::alignment::{AttachmentIdentity, LoadoutAlignment};
pub use dependency_coverage::{
    EquipmentDependencyCoverage, EquipmentEntryFailure, EquipmentFailureStage,
};
pub use loadout_query::{
    AlignmentDependency, AlignmentDependencyState, AlignmentRole, Loadout,
    LoadoutDependencyCoverage, LoadoutOwnerKind, LoadoutPort, LoadoutSnapshot, PortType,
    read_loadouts, read_loadouts_from,
};

#[path = "equipment_ammo_query.rs"]
mod ammo;
#[path = "equipment_attachment_query.rs"]
mod attachments;
#[path = "equipment_bounds_query.rs"]
mod bounds;
#[path = "equipment_cargo_query.rs"]
mod cargo;
#[path = "equipment_cargo_read.rs"]
mod cargo_read;
#[path = "equipment_item_query.rs"]
mod definitions;
#[path = "equipment_item_source_query.rs"]
mod item_sources;
#[path = "equipment_query_read.rs"]
mod read;
#[path = "equipment_query_rows.rs"]
mod rows;
#[path = "equipment_ship_source_query.rs"]
mod ship_sources;
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
pub use crate::catalog_v3::catalog_item_components::selection::ItemSourceSelection;
pub use crate::catalog_v3::catalog_unit_source::UnitSource;
pub use crate::catalog_v3::inventory_dimensions::{InventoryVector2, InventoryVector3};
pub use crate::catalog_v3::inventory_item_occupancy::InventoryItemOccupancy;
pub use crate::catalog_v3::inventory_owner_bounds::{
    BoundsFields as InventoryBoundsFields, InventoryOwnerBounds,
};
pub use crate::item::ItemSource;
pub use ammo::{
    AmmoDefinition, AmmoDefinitionSnapshot, AmmoSelection, with_ammo_definitions,
    with_ammo_definitions_from,
};
pub use attachments::{with_attachment_constraints, with_attachment_constraints_from};
pub use bounds::{with_item_bounds, with_item_bounds_from};
pub use cargo::{
    CapacityStatus, CargoGrid, CargoGridCapacity, CargoGridOwner, CargoGridSnapshot, Facility,
    InventoryContainerDefinition, InventoryContainerType, InventoryItemTypeFilter,
    InventoryOpenContainer, InventoryOwnerStorage, with_cargo_grids, with_cargo_grids_from,
};
pub use definitions::{with_item_definitions, with_item_definitions_from};
#[cfg(test)]
pub(crate) use item_sources::observe_item_source_query;
pub use item_sources::{with_item_sources, with_item_sources_from};
pub use snapshot::{
    AttachmentConstraintSnapshot, EquipmentSnapshot, ItemBoundsSnapshot, ItemDefinitionSnapshot,
    ItemSourceSnapshot,
};

pub use crate::catalog_v3::authored_fields::authored_manufacturer::ManufacturerValue as ShipManufacturerValue;
pub use crate::catalog_v3::authored_fields::ship_presentation::{
    ShipAttachmentManufacturer, ShipVehicleManufacturer,
};
pub use crate::catalog_v3::authored_fields::ship_source_read::ShipPlayabilityInputs;
pub use crate::contract_insurance::{ShipInsurance, ShipInsuranceParams};
pub use crate::ship::ShipSource;
pub use ship_sources::{
    ShipSourceDependencyCoverage, ShipSourceEntryFailure, ShipSourceFailureStage,
    ShipSourceSnapshot, with_ship_sources, with_ship_sources_from,
};
