// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, RecordTree, StructValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_facility::{self, Facility, FacilityKind};
use super::catalog_fuel_configurations::{
    FuelConfigurationKind, FuelConfigurationRows, FuelConfigurationTables,
};
use super::catalog_interactions::{self, InteractionConfiguration, InteractionKind};
use super::catalog_item_alignment::{ItemAlignmentKind, ItemAlignmentRows, ItemAlignmentTables};
use super::catalog_resource_component::{self, ResourceComponent};
use super::catalog_resource_connection::{self, ConnectionKind, ResourceConnection};
use super::catalog_vehicle_armor::{self, VehicleArmor};
use super::inventory_entity_configurations::{InventoryEntityRows, InventoryEntityTables};
use super::inventory_item_occupancy::InventoryItemOccupancy;
use super::item_alignment_options::AlignmentOptions;
use super::item_component_owner::ComponentOwner;
use super::resource_network_links::ResourceNetworkLinks;
use super::resource_topology_declarations::ResourceTopologyDeclarations;
use super::room_associations::{self, RoomAssociations};
use super::{inventory_owner_bounds, inventory_owner_storage};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;

#[derive(Default)]
pub(crate) struct EntityConfigurationRows<'source> {
    pub(crate) alignment: ItemAlignmentRows<'source>,
    pub(crate) topology: ResourceTopologyDeclarations<'source>,
    fuel: FuelConfigurationRows<'source>,
    inventory: InventoryEntityRows<'source>,
    interactions: Vec<InteractionConfiguration<'source>>,
    facilities: Vec<Facility<'source>>,
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
                    .push(alignment, owner, source, options, notes)?;
            }
            ConfigurationKind::RoomAssociations => {
                self.inventory.vehicle(&owner, source, notes)?;
                self.topology
                    .rooms
                    .push(RoomAssociations::read(owner, source, notes)?);
            }
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
        self.resources.extend(other.resources);
        self.connections.extend(other.connections);
        self.vehicle_armor.extend(other.vehicle_armor);
        self.numbers.extend(other.numbers);
    }

    pub(crate) fn into_tables(
        mut self,
        forge: &DataForge<'_>,
    ) -> EntityConfigurationTables<'source> {
        self.facilities.sort_by_key(|row| row.owner().key());
        self.resources.sort_by_key(|row| row.owner().key());
        self.connections.sort_by_key(|row| row.owner().key());
        self.vehicle_armor.sort_by_key(|row| row.owner().key());
        EntityConfigurationTables {
            alignment: self.alignment.into_tables(forge),
            fuel: self.fuel.into_tables(forge),
            inventory: self.inventory.into_tables(forge),
            interactions: catalog_interactions::table(forge, self.interactions),
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
            vehicle_armor: CatalogTable::new(
                forge,
                &[catalog_vehicle_armor::COMPONENT],
                self.vehicle_armor,
            ),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct EntityConfigurationTables<'source> {
    #[serde(flatten)]
    alignment: ItemAlignmentTables<'source>,
    #[serde(flatten)]
    fuel: FuelConfigurationTables<'source>,
    #[serde(flatten)]
    inventory: InventoryEntityTables<'source>,
    interactions: CatalogTable<InteractionConfiguration<'source>>,
    facilities: CatalogTable<Facility<'source>>,
    resource_components: CatalogTable<ResourceComponent<'source>>,
    resource_connections: CatalogTable<ResourceConnection<'source>>,
    vehicle_armor: CatalogTable<VehicleArmor<'source>>,
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

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<(EntityConfigurationRows<'source>, Vec<FieldError>), CatalogError> {
    let input = projection.original();
    let mut rows = EntityConfigurationRows::default();
    let mut notes = Vec::new();
    let Some(Value::Array(components)) = tree.field("Components") else {
        return Ok((rows, notes));
    };
    let mut components = components
        .iter()
        .filter_map(|component| {
            let Value::Struct(value) = component else {
                return None;
            };
            ConfigurationKind::of(value, input).map(|kind| (component, kind))
        })
        .peekable();
    if components.peek().is_none() {
        return Ok((rows, notes));
    }
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let context = input
        .origins()
        .record(tree.record_index())
        .with_cancel(cancel);
    let numbers = RefCell::new(Vec::new());
    for (component, configuration) in components {
        originals.check_cancel()?;
        let component = originals.fact(component)?;
        let owner = ComponentOwner::read(tree, &context, &originals, component)?;
        let source = Observed::new(&originals, input, component, &capture, &numbers);
        rows.push_configuration(
            configuration,
            owner,
            source,
            options,
            projection,
            &mut notes,
        )?;
    }
    rows.numbers = numbers.into_inner();
    Ok((rows, notes))
}
