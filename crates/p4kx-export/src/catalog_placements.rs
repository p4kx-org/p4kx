// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, DataForge};
use schemars::JsonSchema;
use serde::Serialize;

use super::catalog_independent_configurations::flight_configuration_tables::FlightConfigurationTables;
use super::facility_instance::FacilityInstance;
use super::facility_loadout_placements::{
    self, InstalledConfiguration, InteriorRoomCoverage, LoadoutPlacements,
};
use super::resource_network_placements::ResourceNetworkPlacementTables;
use super::resource_topology_declarations::ResourceTopologyTables;
use super::starmap_placement::{self, StarmapPlacement};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_rows::spool::RowSpoolError;
use crate::catalog_rows::staged::StagedCatalogTable;
use crate::catalog_table::CatalogTable;
use crate::collected::Collected;
use crate::options::{CatalogRun, CatalogStage};
use crate::schema_fields::every_field_required;
use crate::starmap_containers::Placements;
use crate::starmap_containers::instance::ContainerInstance;
use crate::starmap_containers::jump_endpoint::JumpEndpoint;
use crate::starmap_containers::location_instance::LocationInstance;
use crate::starmap_containers::unresolved::UnresolvedContainerLink;

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
    container_instances: StagedCatalogTable,
    #[schemars(with = "CatalogTable<LocationInstance>")]
    location_instances: StagedCatalogTable,
    #[schemars(with = "CatalogTable<UnresolvedContainerLink>")]
    unresolved_container_links: StagedCatalogTable,
    jump_endpoints: CatalogTable<JumpEndpoint>,
}

impl CatalogPlacements {
    pub(crate) fn staged_row_bytes(&self) -> u128 {
        u128::from(self.container_instances.stored_bytes())
            + u128::from(self.location_instances.stored_bytes())
            + u128::from(self.unresolved_container_links.stored_bytes())
    }

    pub(crate) fn take_row_spool_failure(&self) -> Option<RowSpoolError> {
        self.container_instances
            .take_failure()
            .or_else(|| self.location_instances.take_failure())
            .or_else(|| self.unresolved_container_links.take_failure())
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
        run: CatalogRun<'_, '_>,
    ) -> Result<(CatalogPlacements, ResourceTopologyTables<'source>), CatalogError> {
        let origins = input.origins();
        let options = run.options;
        run.begin(CatalogStage::ResolveCargoCapacities);
        self.entity_configurations.resolve_cargo_capacities(
            forge,
            &self.inventory_containers,
            &origins.uses,
            options,
        )?;
        let cancel = options.cancel;
        run.begin(CatalogStage::ProjectFacilityLoadouts);
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
            run,
        )?;
        run.begin(CatalogStage::ProjectResourceTopology);
        let topology = self.resource_topology(forge, &origins.uses, cancel)?;
        run.begin(CatalogStage::ProjectStarmapPlacements);
        let starmap = std::mem::take(&mut self.starmap_objects);
        let (availability, spools) = placements.take_spools();
        let placements = CatalogPlacements {
            resource_network: topology.placements,
            starmap_placements: starmap_placement::table(forge, starmap, origins)?,
            facility_instances,
            facility_interior_coverage,
            installed_configurations,
            container_instances: StagedCatalogTable::new(availability, spools.containers, cancel),
            location_instances: StagedCatalogTable::new(availability, spools.locations, cancel),
            unresolved_container_links: StagedCatalogTable::new(
                availability,
                spools.unresolved,
                cancel,
            ),
            jump_endpoints: placements.take_jump_endpoints(),
        };
        Ok((placements, topology.declarations))
    }
}
