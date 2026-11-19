// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter, SourceUses, StructValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_facility::{self, Facility, FacilityKind, OwnerComponents};
pub(crate) use super::catalog_fuel_configurations::FuelConfigurationKind;
use super::catalog_fuel_configurations::{FuelConfigurationRows, FuelConfigurationTables};
use super::catalog_interactions::{self, InteractionConfiguration, InteractionKind};
use super::catalog_item_alignment::{ItemAlignmentKind, ItemAlignmentRows, ItemAlignmentTables};
use super::catalog_resource_component;
use super::catalog_resource_connection::{self, ConnectionKind};
use super::catalog_vehicle_armor;
use super::inventory_container_definition::PreparedInventoryContainer;
use super::inventory_entity_configurations::{InventoryEntityRows, InventoryEntityTables};
use super::inventory_item_occupancy::InventoryItemOccupancy;
use super::item_alignment_options::AlignmentOptions;
use super::item_component_owner::ComponentOwner;
use super::resource_network_links::ResourceNetworkLinks;
use super::resource_topology_declarations::{
    self as topology, ItemRoomAssociation, ResourceTopologyDeclarations,
};
use super::room_associations::{self, RoomAssociations};
use super::{inventory_owner_bounds, inventory_owner_storage};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_rows::staged::{OrderedComponentRows, StagedCatalogTable};
use crate::catalog_table::CatalogTable;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError};

#[path = "catalog_configuration_record_read.rs"]
mod record_read;
pub(crate) use record_read::{
    read, read_attachment_constraints, read_cargo_configurations, read_fuel_intakes,
    read_fuel_tanks, read_item_bounds, read_item_ports, read_item_ports_scoped,
    read_resource_components, read_resource_connections, read_resource_containers,
    read_vehicle_armor,
};

pub use super::authored_damage::{DamageChannels, DamageParameters};
pub use super::authored_link::HealthLink;
pub use super::catalog_fuel::{FuelAudioRtpc, FuelAudioTrigger, FuelIntake, FuelTank};
pub use super::catalog_resource_component::{ResourceComponent, ResourceControls, SelfRepair};
pub use super::catalog_resource_connection::{
    ConnectionFields as ResourceConnectionFields, ResourceConnection,
};
pub use super::catalog_resource_container::{
    CompositionEntry as ResourceCompositionEntry, ResourceContainer,
};
pub use super::catalog_resource_control_parameter::{ParameterSource, ResourceControlParameter};
pub use super::catalog_resource_delta::{ConsumptionFields, DeltaSource, ResourceDelta};
pub use super::catalog_resource_delta_amount::{
    CompositionValue, ResourceComposition, ResourceDeltaAmount,
};
pub use super::catalog_resource_flow_unit::ResourceFlowUnit;
pub use super::catalog_resource_heat_connection::{
    HeatCooling, HeatPerformance, HeatTransfer, ResourceHeatConnection,
};
pub use super::catalog_resource_power_connection::{
    PowerMisfires, PowerPerformance, PowerPresentation, PowerSupply, ResourcePowerConnection,
};
pub use super::catalog_resource_power_ranges::{PowerRange, ResourcePowerRanges};
pub use super::catalog_resource_signature::{ResourceSignatureParams, SignatureEntry};
pub use super::catalog_resource_state::{LinkedInteraction, ResourceState};
pub use super::catalog_vehicle_armor::VehicleArmor;
pub use super::degradation_curve::{CurvePoint, DegradationCurve};
pub use super::vehicle_armor_damage::{ArmorDamageInfo, ArmorDamageSource, VehicleArmorDamage};
pub use super::vehicle_armor_resistance::{
    ArmorDeflection, ArmorHealthCurve, ArmorPenetrationResistance,
};

pub(crate) const VEHICLE_ARMOR_TYPES: &[&str] = &[catalog_vehicle_armor::COMPONENT];
pub(crate) const RESOURCE_COMPONENT_TYPES: &[&str] = &[catalog_resource_component::COMPONENT];
pub(crate) const RESOURCE_CONNECTION_TYPES: &[&str] = catalog_resource_connection::SOURCE_TYPES;

#[derive(Default)]
pub(crate) struct EntityConfigurationRows<'source> {
    pub(crate) alignment: ItemAlignmentRows<'source>,
    pub(crate) topology: ResourceTopologyDeclarations<'source>,
    fuel: FuelConfigurationRows<'source>,
    inventory: InventoryEntityRows<'source>,
    interactions: Vec<InteractionConfiguration<'source>>,
    staged_interactions: Option<StagedCatalogTable>,
    facilities: Vec<Facility<'source>>,
    cargo_owners: Vec<OwnerComponents>,
    resources: Vec<ResourceComponent<'source>>,
    connections: Vec<ResourceConnection<'source>>,
    vehicle_armor: Vec<VehicleArmor<'source>>,
    pub(crate) numbers: Vec<CapturedNumber>,
}

impl<'source> EntityConfigurationRows<'source> {
    pub(super) fn facility_owners(&self) -> impl Iterator<Item = &ComponentOwner> {
        self.facilities.iter().map(Facility::owner)
    }

    pub(super) fn storage_owners(&self) -> impl Iterator<Item = &ComponentOwner> {
        self.inventory.storage_owners()
    }

    pub(super) fn resources(&self) -> &[ResourceComponent<'source>] {
        &self.resources
    }

    fn push_configuration(
        &mut self,
        configuration: ConfigurationKind,
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        options: &AlignmentOptions<'_>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<(), CatalogError> {
        match configuration {
            ConfigurationKind::Alignment {
                alignment,
                occupancy,
            } => {
                if occupancy {
                    self.inventory.occupancy(owner.clone(), source, notes)?;
                }
                if matches!(alignment, ItemAlignmentKind::Port)
                    && let Some(links) = ResourceNetworkLinks::read(owner.clone(), source, notes)?
                {
                    self.topology.links.push(links);
                }
                self.alignment
                    .push(alignment, owner, source, options, projection, notes)?;
            }
            ConfigurationKind::RoomAssociations => {
                self.inventory.vehicle(&owner, source, notes)?;
                self.topology
                    .rooms
                    .push(RoomAssociations::read(owner, source, notes)?);
            }
            ConfigurationKind::ItemRoomAssociation => self
                .topology
                .item_rooms
                .push(ItemRoomAssociation::read(owner, source, notes)?),
            ConfigurationKind::InventoryStorage => self.inventory.storage(owner, source, notes)?,
            ConfigurationKind::ObjectMetadataBounds => {
                self.inventory.metadata(owner, source, notes)?;
            }
            ConfigurationKind::Interaction(kind) => self.interactions.push(
                InteractionConfiguration::read(kind, owner, source, projection, notes)?,
            ),
            ConfigurationKind::Fuel(fuel) => self.fuel.push(fuel, owner, source, notes)?,
            ConfigurationKind::Resource => self
                .resources
                .push(ResourceComponent::read(owner, source, notes)?),
            ConfigurationKind::Connection(kind) => self
                .connections
                .push(ResourceConnection::read(kind, owner, source, notes)?),
            ConfigurationKind::Facility(kind) => self
                .facilities
                .push(Facility::read(kind, owner, source, projection, notes)?),
            ConfigurationKind::VehicleArmor => self
                .vehicle_armor
                .push(VehicleArmor::read(owner, source, notes)?),
        }
        Ok(())
    }

    pub(crate) fn extend(&mut self, other: Self) {
        self.alignment.extend(other.alignment);
        self.topology.extend(other.topology);
        self.fuel.extend(other.fuel);
        self.inventory.extend(other.inventory);
        self.interactions.extend(other.interactions);
        self.facilities.extend(other.facilities);
        self.cargo_owners.extend(other.cargo_owners);
        self.resources.extend(other.resources);
        self.connections.extend(other.connections);
        self.vehicle_armor.extend(other.vehicle_armor);
        self.numbers.extend(other.numbers);
    }

    pub(crate) fn stage_interactions(
        &mut self,
        staging: &mut OrderedComponentRows<'_>,
    ) -> Result<(), CatalogError> {
        for row in std::mem::take(&mut self.interactions) {
            row.stage(staging)?;
        }
        Ok(())
    }

    pub(crate) fn install_staged_rows(
        &mut self,
        interactions: OrderedComponentRows<'_>,
        bounds: OrderedComponentRows<'_>,
        forge: &DataForge<'_>,
    ) -> Result<(), CatalogError> {
        if !self.interactions.is_empty() || self.staged_interactions.is_some() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        self.staged_interactions =
            Some(interactions.finish(forge, catalog_interactions::SOURCE_TYPES)?);
        self.inventory.install_staged_bounds(bounds, forge)
    }

    pub(crate) fn stage_inventory_bounds(
        &mut self,
        staging: &mut OrderedComponentRows<'_>,
    ) -> Result<(), CatalogError> {
        self.inventory.stage_bounds(staging)
    }

    pub(crate) const fn has_cargo_grids(&self) -> bool {
        !self.cargo_owners.is_empty()
    }

    pub(crate) fn cargo_container_index(
        &self,
        forge: &DataForge<'_>,
    ) -> Result<Option<u32>, CatalogError> {
        let [owner] = self.cargo_owners.as_slice() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        catalog_facility::cargo_target_for_owner(owner, self.inventory.item_storage(), forge)
    }

    pub(crate) fn into_cargo_rows(
        self,
    ) -> (
        Vec<Facility<'source>>,
        Vec<inventory_owner_storage::InventoryOwnerStorage<'source>>,
    ) {
        (self.facilities, self.inventory.into_storage())
    }

    pub(crate) fn resolve_cargo_capacities(
        &mut self,
        forge: &DataForge<'_>,
        containers: &[PreparedInventoryContainer<'source>],
        sources: &SourceUses,
        filter: Option<&Filter>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        catalog_facility::project_cargo_capacity(
            &mut self.facilities,
            &self.cargo_owners,
            self.inventory.item_storage(),
            containers,
            forge,
            sources,
            (filter, cancel),
        )
    }

    pub(crate) fn into_tables(
        mut self,
        forge: &DataForge<'_>,
    ) -> Result<EntityConfigurationTables<'source>, CatalogError> {
        if !self.interactions.is_empty() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        self.facilities.sort_by_key(|row| row.owner().key());
        self.resources.sort_by_key(|row| row.owner().key());
        self.connections.sort_by_key(|row| row.owner().key());
        self.vehicle_armor.sort_by_key(|row| row.owner().key());
        Ok(EntityConfigurationTables {
            alignment: self.alignment.into_tables(forge)?,
            fuel: self.fuel.into_tables(forge),
            inventory: self.inventory.into_tables(forge)?,
            interactions: self.staged_interactions.unwrap_or_else(|| {
                StagedCatalogTable::empty(catalog_interactions::empty_availability(forge))
            }),
            facilities: CatalogTable::new(forge, catalog_facility::SOURCE_TYPES, self.facilities),
            resource_components: CatalogTable::new(
                forge,
                &[catalog_resource_component::COMPONENT],
                self.resources,
            ),
            resource_connections: CatalogTable::new(
                forge,
                catalog_resource_connection::SOURCE_TYPES,
                self.connections,
            ),
            vehicle_armor: CatalogTable::new(forge, VEHICLE_ARMOR_TYPES, self.vehicle_armor),
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct EntityConfigurationTables<'source> {
    #[serde(flatten)]
    pub(crate) alignment: ItemAlignmentTables<'source>,
    #[serde(flatten)]
    fuel: FuelConfigurationTables<'source>,
    #[serde(flatten)]
    inventory: InventoryEntityTables<'source>,
    #[schemars(with = "CatalogTable<InteractionConfiguration<'source>>")]
    pub(crate) interactions: StagedCatalogTable,
    facilities: CatalogTable<Facility<'source>>,
    resource_components: CatalogTable<ResourceComponent<'source>>,
    resource_connections: CatalogTable<ResourceConnection<'source>>,
    vehicle_armor: CatalogTable<VehicleArmor<'source>>,
}

impl EntityConfigurationTables<'_> {
    pub(crate) const fn item_bounds(&self) -> &StagedCatalogTable {
        &self.inventory.item_bounds
    }
}

#[derive(Clone, Copy)]
enum ConfigurationKind {
    Alignment {
        alignment: ItemAlignmentKind,
        occupancy: bool,
    },
    InventoryStorage,
    ObjectMetadataBounds,
    Interaction(InteractionKind),
    Fuel(FuelConfigurationKind),
    Resource,
    Connection(ConnectionKind),
    RoomAssociations,
    ItemRoomAssociation,
    Facility(FacilityKind),
    VehicleArmor,
}

impl ConfigurationKind {
    fn of(value: &StructValue<'_>, input: &OriginalCatalogInput<'_, '_>) -> Option<Self> {
        match value.type_name() {
            catalog_resource_component::COMPONENT => return Some(Self::Resource),
            inventory_owner_storage::ITEM_COMPONENT => return Some(Self::InventoryStorage),
            inventory_owner_bounds::METADATA_COMPONENT => return Some(Self::ObjectMetadataBounds),
            catalog_vehicle_armor::COMPONENT => return Some(Self::VehicleArmor),
            _ => {}
        }
        if let Some(kind) = FacilityKind::of(value.type_name()) {
            return Some(Self::Facility(kind));
        }
        if value.type_name() == room_associations::VEHICLE {
            return Some(Self::RoomAssociations);
        }
        if value.type_name() == topology::ITEM_ROOM {
            return Some(Self::ItemRoomAssociation);
        }
        if let Some(kind) = InteractionKind::of(value.type_name()) {
            return Some(Self::Interaction(kind));
        }
        ItemAlignmentKind::of(value, input)
            .map(|alignment| Self::Alignment {
                alignment,
                occupancy: matches!(alignment, ItemAlignmentKind::Attachment)
                    && InventoryItemOccupancy::applies(value),
            })
            .or_else(|| FuelConfigurationKind::of(value.type_name()).map(Self::Fuel))
            .or_else(|| ConnectionKind::of(value.type_name()).map(Self::Connection))
    }
}
