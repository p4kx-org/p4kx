// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForge};
use schemars::JsonSchema;
use serde::Serialize;

use super::facility_instance::FacilityInstance;
use super::facility_loadout_placements::{self, InteriorRoomCoverage};
use super::resource_network_placements::ResourceNetworkPlacementTables;
use super::resource_topology_declarations::ResourceTopologyTables;
use super::starmap_placement::{self, StarmapPlacement};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_table::CatalogTable;
use crate::collected::Collected;
use crate::schema_fields::every_field_required;
use crate::starmap_containers::Placements;
use crate::starmap_containers::instance::ContainerInstance;
use crate::starmap_containers::location_instance::LocationInstance;

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
    container_instances: CatalogTable<ContainerInstance>,
    location_instances: CatalogTable<LocationInstance>,
}

impl<'source> Collected<'source> {
    pub(crate) fn catalog_placements(
        &mut self,
        archive: &Archive,
        forge: &DataForge<'_>,
        input: &OriginalCatalogInput<'_, '_>,
        placements: &mut Placements,
        cancel: Option<&CancelToken>,
    ) -> Result<(CatalogPlacements, ResourceTopologyTables<'source>), CatalogError> {
        let origins = input.origins();
        let (facility_instances, facility_interior_coverage) = facility_loadout_placements::table(
            archive,
            forge,
            &self.ships,
            &self.loadouts,
            &self.entity_configurations,
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
            container_instances: placements.take_instances(),
            location_instances: placements.take_locations(),
        };
        Ok((placements, topology.declarations))
    }
}
