// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, RecordTree, StructValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_fuel_configurations::{
    FuelConfigurationKind, FuelConfigurationRows, FuelConfigurationTables,
};
use super::catalog_item_alignment::{ItemAlignmentKind, ItemAlignmentRows, ItemAlignmentTables};
use super::catalog_resource_component::{self, ResourceComponent};
use super::catalog_resource_connection::{self, ConnectionKind, ResourceConnection};
use super::catalog_vehicle_armor::{self, VehicleArmor};
use super::item_alignment_options::AlignmentOptions;
use super::item_component_owner::ComponentOwner;
use super::resource_network_links::ResourceNetworkLinks;
use super::resource_topology_declarations::ResourceTopologyDeclarations;
use super::room_associations::{self, RoomAssociations};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_number_capture::CapturedNumber;
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
    resources: Vec<ResourceComponent<'source>>,
    connections: Vec<ResourceConnection<'source>>,
    vehicle_armor: Vec<VehicleArmor<'source>>,
    pub(crate) numbers: Vec<CapturedNumber>,
}

impl<'source> EntityConfigurationRows<'source> {
    pub(super) fn resources(&self) -> &[ResourceComponent<'source>] {
        &self.resources
    }

    pub(crate) fn extend(&mut self, other: Self) {
        self.alignment.extend(other.alignment);
        self.topology.extend(other.topology);
        self.fuel.extend(other.fuel);
        self.resources.extend(other.resources);
        self.connections.extend(other.connections);
        self.vehicle_armor.extend(other.vehicle_armor);
        self.numbers.extend(other.numbers);
    }

    pub(crate) fn into_tables(
        mut self,
        forge: &DataForge<'_>,
    ) -> EntityConfigurationTables<'source> {
        self.resources.sort_by_key(|row| row.owner().key());
        self.connections.sort_by_key(|row| row.owner().key());
        self.vehicle_armor.sort_by_key(|row| row.owner().key());
        EntityConfigurationTables {
            alignment: self.alignment.into_tables(forge),
            fuel: self.fuel.into_tables(forge),
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
    resource_components: CatalogTable<ResourceComponent<'source>>,
    resource_connections: CatalogTable<ResourceConnection<'source>>,
    vehicle_armor: CatalogTable<VehicleArmor<'source>>,
}

enum ConfigurationKind {
    Alignment(ItemAlignmentKind),
    Fuel(FuelConfigurationKind),
    Resource,
    Connection(ConnectionKind),
    RoomAssociations,
    VehicleArmor,
}

impl ConfigurationKind {
    fn of(value: &StructValue<'_>, input: &OriginalCatalogInput<'_, '_>) -> Option<Self> {
        match value.type_name() {
            catalog_resource_component::COMPONENT => return Some(Self::Resource),
            catalog_vehicle_armor::COMPONENT => return Some(Self::VehicleArmor),
            _ => {}
        }
        if value.type_name() == room_associations::VEHICLE {
            return Some(Self::RoomAssociations);
        }
        ItemAlignmentKind::of(value, input)
            .map(Self::Alignment)
            .or_else(|| FuelConfigurationKind::of(value.type_name()).map(Self::Fuel))
            .or_else(|| ConnectionKind::of(value.type_name()).map(Self::Connection))
    }
}

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<(EntityConfigurationRows<'source>, Vec<FieldError>), CatalogError> {
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
        match configuration {
            ConfigurationKind::Alignment(alignment) => {
                if matches!(alignment, ItemAlignmentKind::Port)
                    && let Some(links) =
                        ResourceNetworkLinks::read(owner.clone(), source, &mut notes)?
                {
                    rows.topology.links.push(links);
                }
                rows.alignment
                    .push(alignment, owner, source, options, &mut notes)?;
            }
            ConfigurationKind::RoomAssociations => rows
                .topology
                .rooms
                .push(RoomAssociations::read(owner, source, &mut notes)?),
            ConfigurationKind::Fuel(fuel) => rows.fuel.push(fuel, owner, source, &mut notes)?,
            ConfigurationKind::Resource => rows
                .resources
                .push(ResourceComponent::read(owner, source, &mut notes)?),
            ConfigurationKind::Connection(kind) => rows
                .connections
                .push(ResourceConnection::read(kind, owner, source, &mut notes)?),
            ConfigurationKind::VehicleArmor => rows
                .vehicle_armor
                .push(VehicleArmor::read(owner, source, &mut notes)?),
        }
    }
    rows.numbers = numbers.into_inner();
    Ok((rows, notes))
}
