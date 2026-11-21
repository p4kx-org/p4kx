// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective queries over original equipment, resolved Ships and Loadouts.

#[path = "equipment_component_query.rs"]
mod component_query;
#[path = "equipment_dependency_coverage.rs"]
mod dependency_coverage;

/// Selective reads of authored cargo loading times and shop commodity fees.
#[path = "equipment_cargo_globals_query.rs"]
pub mod cargo_globals;

/// Canonical definitions referenced by consumable, mining and armor components.
#[path = "equipment_component_definitions.rs"]
pub mod component_definitions;

/// Selective reads of authored Health, degradation and distortion components.
#[path = "equipment_durability_query.rs"]
pub mod durability;

/// Selective reads of authored consumable components and contents slots.
#[path = "equipment_consumable_query.rs"]
pub mod consumables;

/// Selective reads of declared contract scope labels and icons.
#[path = "equipment_contract_scope_query.rs"]
pub mod contract_scopes;
/// Selective reads of authored Mannequin controller and animation database locators.
#[path = "equipment_entity_animation_query.rs"]
pub mod entity_animation;

#[path = "equipment_loadout_query.rs"]
mod loadout_query;

/// Selective reads of authored flight-controller, IFCS and thruster configurations.
#[path = "equipment_flight_query.rs"]
pub mod flight;

/// Selective reads of original EMP, ordnance, missile and explosion data.
#[path = "equipment_ordnance_query.rs"]
pub mod ordnance;

/// Selective reads of original fuel tanks, intakes and resource containers.
#[path = "equipment_fuel_query.rs"]
pub mod fuel;

/// Selective reads of physical hacking chips and hackable devices.
#[path = "equipment_hacking_query.rs"]
pub mod hacking;

/// Selective reads of authored loot-generation components and their original values.
#[path = "equipment_loot_generation_query.rs"]
pub mod loot_generation;

/// Selective reads of authored mining lasers and attachable modifiers.
#[path = "equipment_mining_tool_query.rs"]
pub mod mining_tools;

/// Selective reads of mission board providers and their original board types.
#[path = "equipment_mission_board_query.rs"]
pub mod mission_boards;

/// Selective reads of authored jump-point configurations on physical world Entities.
#[path = "equipment_jump_point_query.rs"]
pub mod jump_points;

/// Selective reads of authored ship shield-generator and quantum-drive configurations.
#[path = "equipment_ship_system_query.rs"]
pub mod ship_systems;

#[path = "equipment_record_query.rs"]
mod record_query;
/// Direct queries for original resource components and their power and heat connections.
#[path = "equipment_resource_network_query.rs"]
pub mod resource_network;
/// Selective reads of original resource links and room declarations.
#[path = "equipment_resource_topology_query.rs"]
pub mod resource_topology;

/// Selective reads of authored vehicle armor and its original components.
#[path = "equipment_vehicle_armor_query.rs"]
pub mod vehicle_armor;

/// Selective reads of authored clothing, suit armor and per-item radar components.
#[path = "equipment_worn_gear_query.rs"]
pub mod worn_gear;

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
#[path = "equipment_facility_query.rs"]
mod facilities;
#[path = "equipment_faction_query.rs"]
pub mod factions;
#[path = "equipment_inventory_query.rs"]
mod inventory;
#[path = "equipment_item_source_query.rs"]
mod item_sources;
#[path = "equipment_planet_audio_query.rs"]
pub mod planet_audio;
#[path = "equipment_projectile_ammo_query.rs"]
mod projectile_ammo;
#[path = "equipment_projectile_ammo_read.rs"]
mod projectile_ammo_read;
#[path = "equipment_query_read.rs"]
mod read;
#[path = "equipment_query_rows.rs"]
mod rows;
#[path = "equipment_ship_source_query.rs"]
mod ship_sources;
#[path = "equipment_query_snapshot.rs"]
mod snapshot;
#[path = "equipment_weapon_query.rs"]
mod weapon;
#[path = "equipment_weapon_animation_sources_query.rs"]
pub mod weapon_animation_sources;

#[path = "equipment_weapon_animation_query.rs"]
mod weapon_animation;
#[path = "equipment_weapon_combat_query.rs"]
mod weapon_combat;
#[path = "equipment_weapon_combat_read.rs"]
mod weapon_combat_read;
#[path = "equipment_weapon_magazine_query.rs"]
mod weapon_magazine;
#[path = "equipment_weapon_magazine_dependency_read.rs"]
mod weapon_magazine_dependency_read;
#[path = "equipment_weapon_recoil_query.rs"]
mod weapon_recoil;
#[path = "equipment_weapon_query_rows.rs"]
mod weapon_rows;

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
pub use facilities::{
    FacilityKind, FacilityOwner, FacilitySnapshot, with_facilities, with_facilities_from,
};
pub use inventory::{
    InventoryConfigurationSnapshot, InventoryContainerSnapshot, with_inventory_configurations,
    with_inventory_configurations_from, with_inventory_containers, with_inventory_containers_from,
};
#[cfg(test)]
pub(crate) use item_sources::observe_item_source_query;
pub use item_sources::{with_item_sources, with_item_sources_from};
pub use projectile_ammo::{
    CountermeasureEmitter, CountermeasureEmitterRoute, ProjectileAmmoChoice, ProjectileAmmoOwner,
    ProjectileAmmoRole, ProjectileAmmoSnapshot, WeaponProjectileAmmoSelection,
    with_projectile_ammo, with_projectile_ammo_from,
};
pub use snapshot::{
    AttachmentConstraintSnapshot, EquipmentSnapshot, ItemBoundsSnapshot, ItemDefinitionSnapshot,
    ItemSourceSnapshot,
};

pub use weapon::{
    AmmoBinding, AmmoContainer, ContainerAmmoSlots, ProjectileCombatInputs, SecondaryMagazine,
    WeaponAction, WeaponActions, WeaponMagazineSlots, WeaponModeKind, WeaponSource,
    WeaponSourceSnapshot, with_weapon_sources, with_weapon_sources_from,
};

pub use weapon_combat::{
    AlphaUnit, CombatModel, CombatUnits, DamageChannels, DistanceUnit, DpsUnit, FireRateUnit,
    NominalBullet, NominalBulletDamage, WeaponCombatBase, WeaponCombatOwner, WeaponCombatSnapshot,
    with_weapon_combat_bases, with_weapon_combat_bases_from,
};

pub use weapon_magazine::{
    LinkedMagazineSources, WeaponMagazineDependencyOwner, WeaponMagazineDependencySnapshot,
    WeaponMagazineOutcome, WeaponMagazineResolution, WeaponMagazineSnapshot,
    with_weapon_magazine_dependencies, with_weapon_magazine_dependencies_from,
    with_weapon_magazine_resolutions, with_weapon_magazine_resolutions_from,
};

pub use crate::catalog_v3::catalog_weapon_actions::WeaponActionRecoil;
pub use weapon_animation::{
    WeaponAnimationGraphCoverage, WeaponAnimationPlayback, WeaponAnimationSnapshot,
    WeaponAnimationTagEvidence, WeaponAnimationTagMatch, WeaponPlayerAnimationCandidate,
    with_weapon_animation_candidates, with_weapon_animation_candidates_from,
};
pub use weapon_recoil::{
    WeaponActionRecoilDefinition, WeaponRecoilSnapshot, with_weapon_recoil_definitions,
    with_weapon_recoil_definitions_from,
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
