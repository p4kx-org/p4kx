// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{Archive, CancelToken, DataForge};
use schemars::JsonSchema;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

use super::catalog_independent_configurations::flight_configuration_tables::FlightConfigurationTables;
use super::facility_instance::FacilityInstance;
use super::facility_loadout_placements::{
    self, InstalledConfiguration, InteriorRoomCoverage, LoadoutPlacements,
};
use super::resource_network_placements::ResourceNetworkPlacementTables;
use super::resource_topology_declarations::ResourceTopologyTables;
use super::starmap_placement::{self, StarmapPlacement};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_rows::spool::{RowSpool, RowSpoolError};
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::collected::Collected;
use crate::schema_fields::every_field_required;
use crate::starmap_containers::Placements;
use crate::starmap_containers::instance::ContainerInstance;
use crate::starmap_containers::jump_endpoint::JumpEndpoint;
use crate::starmap_containers::location_instance::LocationInstance;
use crate::{CatalogError, CatalogOptions};

/// Contextual placements and relationships keyed by their physical definitions.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CatalogPlacements {
    #[serde(flatten)]
    resource_network: ResourceNetworkPlacementTables,
    starmap_placements: CatalogTable<StarmapPlacement>,
    facility_instances: CatalogTable<FacilityInstance>,
    facility_interior_coverage: Vec<InteriorRoomCoverage>,
    installed_configurations: CatalogTable<InstalledConfiguration>,
    #[schemars(with = "CatalogTable<ContainerInstance>")]
    container_instances: ContainerInstances,
    location_instances: CatalogTable<LocationInstance>,
    jump_endpoints: CatalogTable<JumpEndpoint>,
}

#[derive(Debug)]
struct ContainerInstances {
    availability: DatasetAvailability,
    spool: Option<RowSpool>,
    cancel: Option<CancelToken>,
    failure: RefCell<Option<RowSpoolError>>,
}

impl ContainerInstances {
    fn take(placements: &mut Placements, cancel: Option<&CancelToken>) -> Self {
        let (availability, spool) = placements.take_instance_spool();
        Self {
            availability,
            spool,
            cancel: cancel.cloned(),
            failure: RefCell::new(None),
        }
    }
}

impl Serialize for ContainerInstances {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        *self.failure.borrow_mut() = None;
        let mut table = serializer.serialize_struct("CatalogTable", 2)?;
        table.serialize_field("availability", &self.availability)?;
        if let Some(spool) = &self.spool {
            let rows = spool.serialize_rows(self.cancel.as_ref());
            let result = table.serialize_field("rows", &rows);
            *self.failure.borrow_mut() = rows.take_failure();
            result?;
        } else {
            table.serialize_field("rows", &[] as &[ContainerInstance])?;
        }
        table.end()
    }
}

impl CatalogPlacements {
    pub(crate) fn take_row_spool_failure(&self) -> Option<RowSpoolError> {
        self.container_instances.failure.take()
    }
}

impl<'source> Collected<'source> {
    pub(crate) fn catalog_placements(
        &mut self,
        archive: &Archive,
        forge: &DataForge<'_>,
        input: &OriginalCatalogInput<'_, '_>,
        placements: &mut Placements,
        flight: &FlightConfigurationTables<'_>,
        options: &CatalogOptions<'_>,
    ) -> Result<(CatalogPlacements, ResourceTopologyTables<'source>), CatalogError> {
        let origins = input.origins();
        self.entity_configurations.resolve_cargo_capacities(
            forge,
            &self.inventory_containers,
            &origins.uses,
            options,
        )?;
        let cancel = options.cancel;
        let LoadoutPlacements {
            facilities: facility_instances,
            rooms: facility_interior_coverage,
            flight: installed_configurations,
        } = facility_loadout_placements::table(
            archive,
            forge,
            &self.ships,
            &self.loadouts,
            facility_loadout_placements::ConfigurationSources {
                entities: &self.entity_configurations,
                flight,
            },
            input,
            cancel,
        )?;
        let topology = self.resource_topology(forge, &origins.uses, cancel)?;
        let starmap = std::mem::take(&mut self.starmap_objects);
        let placements = CatalogPlacements {
            resource_network: topology.placements,
            starmap_placements: starmap_placement::table(forge, starmap, origins)?,
            facility_instances,
            facility_interior_coverage,
            installed_configurations,
            container_instances: ContainerInstances::take(placements, cancel),
            location_instances: placements.take_locations(),
            jump_endpoints: placements.take_jump_endpoints(),
        };
        Ok((placements, topology.declarations))
    }
}
