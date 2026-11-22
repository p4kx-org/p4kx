// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{CancelToken, RecordTree, StructValue, Value};

use super::super::{
    ConfigurationKind, EntityConfigurationRows, FuelConfigurationKind, ResourceTopologyKind,
};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_facility::{CARGO_GRID_SOURCE_TYPE, FacilityKind};
use crate::catalog_v3::catalog_item_alignment::{
    ATTACHABLE, AttachmentTarget, CONTAINER, ItemAlignmentKind, attachment_target,
};
use crate::catalog_v3::item_alignment_options::AlignmentOptions;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::catalog_v3::{inventory_owner_storage, room_associations};
use crate::export_field_inspection::ConfigurationInspection;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError};

#[derive(Clone, Copy)]
pub(super) enum Scope {
    All,
    Ports,
    Attachments,
    Bounds,
    CargoGrids,
    Facilities,
    FacilitiesWithCargoStorage,
    LoadoutFacilityOwners,
    InventoryConfigurations,
    VehicleArmor,
    ResourceComponents,
    ResourceConnections,
    ResourceTopology(ResourceTopologyKind),
    Fuel(FuelConfigurationKind),
}

pub(super) struct ConfigurationOwner {
    pub(super) configuration: ConfigurationKind,
    pub(super) owner: ComponentOwner,
}

impl Scope {
    pub(super) fn configuration(
        self,
        value: &StructValue<'_>,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Option<ConfigurationKind> {
        if matches!(self, Self::Ports) {
            return (value.type_name() == CONTAINER).then_some(ConfigurationKind::Alignment {
                alignment: ItemAlignmentKind::Port,
                occupancy: false,
            });
        }
        if matches!(self, Self::InventoryConfigurations) {
            return match value.type_name() {
                inventory_owner_storage::ITEM_COMPONENT => {
                    Some(ConfigurationKind::InventoryStorage)
                }
                room_associations::VEHICLE => Some(ConfigurationKind::RoomAssociations),
                _ => None,
            };
        }
        let kind = ConfigurationKind::of(value, input)?;
        self.accepts(kind).then_some(kind)
    }

    pub(super) fn for_owner(self, components: &[Value<'_>]) -> Option<Self> {
        if !matches!(self, Self::CargoGrids | Self::Facilities) {
            return Some(self);
        }
        let has_cargo_grid = components.iter().any(|slot| {
            matches!(slot, Value::Struct(component) if component.type_name() == CARGO_GRID_SOURCE_TYPE)
        });
        match (self, has_cargo_grid) {
            (Self::CargoGrids, false) => None,
            (Self::Facilities, true) => Some(Self::FacilitiesWithCargoStorage),
            _ => Some(self),
        }
    }

    fn accepts(self, kind: ConfigurationKind) -> bool {
        match self {
            Self::All => true,
            Self::Attachments => matches!(
                kind,
                ConfigurationKind::Alignment {
                    alignment: ItemAlignmentKind::Attachment,
                    ..
                }
            ),
            Self::Bounds => matches!(
                kind,
                ConfigurationKind::Alignment {
                    occupancy: true,
                    ..
                } | ConfigurationKind::RoomAssociations
                    | ConfigurationKind::ObjectMetadataBounds
            ),
            Self::CargoGrids => matches!(
                kind,
                ConfigurationKind::Facility(FacilityKind::CargoGrid)
                    | ConfigurationKind::InventoryStorage
            ),
            Self::Facilities => matches!(kind, ConfigurationKind::Facility(_)),
            Self::FacilitiesWithCargoStorage => matches!(
                kind,
                ConfigurationKind::Facility(_) | ConfigurationKind::InventoryStorage
            ),
            Self::LoadoutFacilityOwners => matches!(
                kind,
                ConfigurationKind::Facility(_)
                    | ConfigurationKind::InventoryStorage
                    | ConfigurationKind::RoomAssociations
            ),
            Self::VehicleArmor => matches!(kind, ConfigurationKind::VehicleArmor),
            Self::ResourceComponents => matches!(kind, ConfigurationKind::Resource),
            Self::ResourceConnections => matches!(kind, ConfigurationKind::Connection(_)),
            Self::ResourceTopology(declaration) => matches!(
                (declaration, kind),
                (
                    ResourceTopologyKind::Links,
                    ConfigurationKind::Alignment {
                        alignment: ItemAlignmentKind::Port,
                        ..
                    }
                ) | (
                    ResourceTopologyKind::VehicleRooms,
                    ConfigurationKind::RoomAssociations
                ) | (
                    ResourceTopologyKind::ItemRooms,
                    ConfigurationKind::ItemRoomAssociation
                )
            ),
            Self::Fuel(requested) => {
                matches!(kind, ConfigurationKind::Fuel(actual) if actual == requested)
            }
            Self::Ports | Self::InventoryConfigurations => false,
        }
    }

    pub(super) const fn inspection(self) -> Option<ConfigurationInspection> {
        match self {
            Self::VehicleArmor => Some(ConfigurationInspection::VehicleArmor),
            Self::ResourceComponents => Some(ConfigurationInspection::ResourceComponent),
            Self::ResourceConnections => Some(ConfigurationInspection::ResourceConnection),
            Self::ResourceTopology(declaration) => Some(declaration.inspection()),
            Self::Fuel(FuelConfigurationKind::Tank) => Some(ConfigurationInspection::FuelTank),
            Self::Fuel(FuelConfigurationKind::Intake) => Some(ConfigurationInspection::FuelIntake),
            Self::Fuel(FuelConfigurationKind::Container) => {
                Some(ConfigurationInspection::ResourceContainer)
            }
            _ => None,
        }
    }

    pub(super) const fn inspected_members(self) -> Option<&'static [&'static str]> {
        match self {
            Self::ResourceTopology(declaration) => declaration.inspected_members(),
            _ => None,
        }
    }

    pub(super) fn append_inspection_spans<'scope, 'read, 'source>(
        self,
        originals: &OriginalFlowFields<'scope, 'read, 'source>,
        component: &OriginalValueFact<'scope, 'read, 'source>,
        spans: &mut Vec<Range<usize>>,
    ) -> Result<(), CatalogError> {
        if let Some(members) = self.inspected_members() {
            for name in members {
                if let Some((_, field)) = originals.field(Some(component), name)? {
                    spans.push(originals.span(field)?);
                }
            }
        } else {
            spans.push(originals.span(component)?);
        }
        Ok(())
    }

    pub(super) fn selection_complete(
        self,
        tree: &RecordTree<'_>,
        input: &OriginalCatalogInput<'_, '_>,
        cancel: Option<&CancelToken>,
    ) -> Result<bool, CatalogError> {
        if matches!(self, Self::All) {
            return Ok(false);
        }
        let Some(value) = tree.field("Components") else {
            return Ok(true);
        };
        let Value::Array(components) = value else {
            return Ok(matches!(value, Value::Null));
        };
        for value in components {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            let complete = match value {
                Value::Error(_) => false,
                Value::Struct(component)
                    if matches!(self, Self::Attachments | Self::Bounds)
                        && component.type_name() == ATTACHABLE =>
                {
                    self.attachment_complete(component, input)
                }
                Value::Struct(_) | Value::Null => true,
                _ => !matches!(
                    self,
                    Self::CargoGrids
                        | Self::Facilities
                        | Self::FacilitiesWithCargoStorage
                        | Self::LoadoutFacilityOwners
                        | Self::InventoryConfigurations
                ),
            };
            if !complete {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn attachment_complete(
        self,
        component: &StructValue<'_>,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> bool {
        if matches!(self, Self::Bounds) && matches!(component.field("AttachDef"), Some(Value::Null))
        {
            return true;
        }
        match attachment_target(component, input) {
            AttachmentTarget::Unknown => false,
            AttachmentTarget::Definition if matches!(self, Self::Bounds) => {
                !matches!(component.field("AttachDef"), Some(Value::Error(_)))
            }
            _ => true,
        }
    }

    pub(super) fn project<'source>(
        self,
        rows: &mut EntityConfigurationRows<'source>,
        selected: ConfigurationOwner,
        source: Observed<'_, '_, '_, 'source>,
        options: &AlignmentOptions<'_>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<(), CatalogError> {
        let ConfigurationOwner {
            configuration,
            owner,
        } = selected;
        match self {
            Self::LoadoutFacilityOwners
                if matches!(configuration, ConfigurationKind::RoomAssociations) =>
            {
                rows.inventory.vehicle_storage(&owner, source, notes)
            }
            Self::All
            | Self::CargoGrids
            | Self::Facilities
            | Self::FacilitiesWithCargoStorage
            | Self::LoadoutFacilityOwners
            | Self::VehicleArmor
            | Self::ResourceComponents
            | Self::ResourceConnections
            | Self::Fuel(_) => {
                rows.push_configuration(configuration, owner, source, options, projection, notes)
            }
            Self::ResourceTopology(declaration) => {
                rows.topology.project(declaration, owner, source, notes)
            }
            Self::Ports | Self::Attachments => {
                let ConfigurationKind::Alignment { alignment, .. } = configuration else {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                };
                rows.alignment
                    .push(alignment, owner, source, options, projection, notes)
            }
            Self::InventoryConfigurations => match configuration {
                ConfigurationKind::InventoryStorage => rows.inventory.storage(owner, source, notes),
                ConfigurationKind::RoomAssociations => {
                    rows.inventory.vehicle_storage(&owner, source, notes)
                }
                _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
            },
            Self::Bounds => match configuration {
                ConfigurationKind::Alignment {
                    occupancy: true, ..
                } => rows.inventory.occupancy(owner, source, notes),
                ConfigurationKind::RoomAssociations => {
                    rows.inventory.vehicle_bounds(&owner, source, notes)
                }
                ConfigurationKind::ObjectMetadataBounds => {
                    rows.inventory.metadata(owner, source, notes)
                }
                _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
            },
        }
    }
}
