// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{CancelToken, DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::catalog_entity_component_read::PreparedComponent;
use super::super::catalog_independent_configurations::flight_configuration_tables::{
    FLIGHT_CONTROLLERS, FlightConfigurationTables, FlightOwner, IFCS, THRUSTERS,
};
use super::super::catalog_item_alignment::PhysicalPort;
use super::super::facility_instance::{LoadoutFacilityPlacement, ShipContext};
use super::super::item_component_owner::ComponentOwner;
use super::super::loadout_position::LoadoutPosition;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::component_sources::flight::{FlightController, Ifcs, Thruster};
use crate::guid_text::CatalogGuid;
use crate::loadout::alignment::{AttachmentIdentity, LoadoutAlignment};
use crate::schema_fields::every_field_required;
use crate::{CatalogError, ExportAccountingError};

pub use super::super::catalog_independent_configurations::flight_configuration_tables::FlightConfigurationKind;

/// Original flight configurations on one resolved vehicle Loadout occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct InstalledConfiguration {
    #[serde(flatten)]
    vehicle: ShipContext,
    source: LoadoutFacilityPlacement,
    #[schemars(length(min = 1))]
    configurations: Vec<FlightConfigurationLink>,
}

impl InstalledConfiguration {
    /// Borrows the GUID of the physical vehicle whose resolved loadout contains the occurrence.
    #[must_use]
    pub const fn ship_guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.vehicle.ship_guid
    }

    /// Borrows the original physical Record index of the vehicle.
    #[must_use]
    pub const fn ship_record_index(&self) -> &SourcedValue<u32> {
        &self.vehicle.ship_record_index
    }

    /// Borrows the exact root and port-ordinal path.
    #[must_use]
    pub const fn position(&self) -> &SourcedValue<LoadoutPosition> {
        &self.source.position
    }

    /// Borrows the physical port matched for an attached configuration.
    #[must_use]
    pub const fn original_port(&self) -> Option<&PhysicalPort> {
        self.source.original_port.as_ref()
    }

    /// Borrows the physical attachment when the occurrence is below its root vehicle.
    #[must_use]
    pub const fn attached_item(&self) -> Option<&AttachmentIdentity> {
        self.source.attached_item.as_ref()
    }

    /// Borrows the original flight configuration owners in component order.
    #[must_use]
    pub fn configurations(&self) -> &[FlightConfigurationLink] {
        &self.configurations
    }
}

/// A flight configuration on the physical owner selected by the loadout occurrence.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[schemars(description = "", transform = every_field_required)]
pub struct FlightConfigurationLink {
    kind: FlightConfigurationKind,
    owner: ComponentOwner,
}

impl FlightConfigurationLink {
    /// Returns the authored configuration family.
    #[must_use]
    pub const fn kind(&self) -> FlightConfigurationKind {
        self.kind
    }

    /// Borrows the original Record and component ordinal.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }
}

pub(crate) type InstalledFlightLookup<'scope> =
    dyn FnMut(u32) -> Result<Vec<FlightConfigurationLink>, CatalogError> + 'scope;

enum Definitions<'rows> {
    Indexed(HashMap<u32, Vec<FlightOwner<'rows>>>),
    Scoped(&'rows mut InstalledFlightLookup<'rows>),
}

pub(super) struct InstalledConfigurations<'rows> {
    definitions: Definitions<'rows>,
    pub(super) rows: Vec<InstalledConfiguration>,
}

impl Default for InstalledConfigurations<'_> {
    fn default() -> Self {
        Self {
            definitions: Definitions::Indexed(HashMap::new()),
            rows: Vec::new(),
        }
    }
}

impl<'rows> InstalledConfigurations<'rows> {
    pub(super) fn scoped(lookup: &'rows mut InstalledFlightLookup<'rows>) -> Self {
        Self {
            definitions: Definitions::Scoped(lookup),
            rows: Vec::new(),
        }
    }

    pub(super) fn new(tables: &'rows FlightConfigurationTables<'_>) -> Result<Self, CatalogError> {
        let mut definitions = HashMap::<u32, Vec<FlightOwner<'_>>>::new();
        for definition in tables.owners() {
            let (Some(record), Some(_)) = definition.owner.key() else {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            };
            definitions.entry(record).or_default().push(definition);
        }
        for definitions in definitions.values_mut() {
            definitions.sort_by_key(|definition| definition.owner.key().1);
        }
        Ok(Self {
            definitions: Definitions::Indexed(definitions),
            rows: Vec::new(),
        })
    }

    pub(super) fn is_empty(&self) -> bool {
        matches!(&self.definitions, Definitions::Indexed(index) if index.is_empty())
    }

    pub(super) fn links(
        &mut self,
        record: u32,
    ) -> Result<Vec<FlightConfigurationLink>, CatalogError> {
        Ok(match &mut self.definitions {
            Definitions::Indexed(definitions) => {
                definitions.get(&record).map_or_else(Vec::new, |rows| {
                    rows.iter().map(FlightConfigurationLink::new).collect()
                })
            }
            Definitions::Scoped(lookup) => lookup(record)?,
        })
    }

    pub(super) fn emit(
        &mut self,
        vehicle: &ShipContext,
        configurations: Vec<FlightConfigurationLink>,
        position: impl FnOnce() -> SourcedValue<LoadoutPosition>,
        attachment: Option<(&LoadoutAlignment, &AttachmentIdentity)>,
    ) -> Result<(), CatalogError> {
        if configurations.is_empty() {
            return Ok(());
        }
        let (original_port, attached_item) = if let Some((alignment, attachment)) = attachment {
            (alignment.physical_port()?, Some(attachment.clone()))
        } else {
            (None, None)
        };
        self.rows.push(InstalledConfiguration {
            vehicle: vehicle.clone(),
            source: LoadoutFacilityPlacement {
                position: position(),
                original_port,
                attached_item,
            },
            configurations,
        });
        Ok(())
    }
}

impl FlightConfigurationLink {
    fn new(definition: &FlightOwner<'_>) -> Self {
        Self {
            kind: definition.kind,
            owner: definition.owner.clone(),
        }
    }

    fn sort_for_owner(links: &mut [Self], record_index: u32) -> Result<(), CatalogError> {
        if links.iter().any(|link| {
            let (index, ordinal) = link.owner.key();
            index != Some(record_index) || ordinal.is_none()
        }) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        links.sort_by_key(|link| link.owner.key().1);
        Ok(())
    }
}

/// Read coverage for the three flight component families on a reached physical Record.
#[derive(Debug)]
#[non_exhaustive]
pub struct InstalledConfigurationDependency {
    /// The original physical Record index.
    pub record_index: u32,
    /// Flight Controller header, body and field coverage.
    pub flight_controllers: CatalogRecordCoverage,
    /// IFCS header, body and field coverage.
    pub ifcs: CatalogRecordCoverage,
    /// Thruster header, body and field coverage.
    pub thrusters: CatalogRecordCoverage,
}

pub(crate) struct ScopedFlightLinks {
    pub(crate) links: Vec<FlightConfigurationLink>,
    pub(crate) coverage: InstalledConfigurationDependency,
}

pub(crate) struct FlightComponentReaders<'source> {
    controllers: PreparedComponent<'source>,
    ifcs: PreparedComponent<'source>,
    thrusters: PreparedComponent<'source>,
}

impl<'source> FlightComponentReaders<'source> {
    pub(crate) const fn new(forge: &'source DataForge<'source>) -> Self {
        Self {
            controllers: PreparedComponent::new(forge, FLIGHT_CONTROLLERS),
            ifcs: PreparedComponent::new(forge, IFCS),
            thrusters: PreparedComponent::new(forge, THRUSTERS),
        }
    }

    pub(crate) fn read_owner_links(
        &self,
        input: &OriginalCatalogInput<'_, 'source>,
        record_index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedFlightLinks, CatalogError> {
        let (controllers, flight_controllers) = self.controllers.read_with_partials(
            input,
            record_index,
            cancel,
            FlightController::read,
        )?;
        let (systems, ifcs) =
            self.ifcs
                .read_with_partials(input, record_index, cancel, Ifcs::read)?;
        let (engines, thrusters) =
            self.thrusters
                .read_with_partials(input, record_index, cancel, Thruster::read)?;
        let owners = controllers
            .iter()
            .map(|row| FlightOwner {
                kind: FlightConfigurationKind::FlightController,
                owner: row.owner(),
            })
            .chain(systems.iter().map(|row| FlightOwner {
                kind: FlightConfigurationKind::Ifcs,
                owner: row.owner(),
            }))
            .chain(engines.iter().map(|row| FlightOwner {
                kind: FlightConfigurationKind::Thruster,
                owner: row.owner(),
            }));
        let mut links = owners
            .map(|owner| FlightConfigurationLink::new(&owner))
            .collect::<Vec<_>>();
        FlightConfigurationLink::sort_for_owner(&mut links, record_index)?;
        Ok(ScopedFlightLinks {
            links,
            coverage: InstalledConfigurationDependency {
                record_index,
                flight_controllers,
                ifcs,
                thrusters,
            },
        })
    }
}

pub(crate) fn scoped_table(
    forge: &DataForge<'_>,
    rows: Vec<InstalledConfiguration>,
) -> CatalogTable<InstalledConfiguration> {
    FlightConfigurationTables::installed_table(forge, rows)
}
