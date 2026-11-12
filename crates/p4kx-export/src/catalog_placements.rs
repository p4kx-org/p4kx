// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge};
use schemars::JsonSchema;
use serde::Serialize;

use super::facility_instance::FacilityInstance;
use super::facility_loadout_placements;
use super::resource_network_placements::ResourceNetworkPlacementTables;
use super::resource_topology_declarations::ResourceTopologyTables;
use super::starmap_placement::{self, StarmapPlacement};
use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_table::CatalogTable;
use crate::collected::Collected;
use crate::schema_fields::every_field_required;
use crate::starmap_containers::instance::ContainerInstance;

/// Contextual placements and relationships keyed by their physical definitions.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CatalogPlacements {
    #[serde(flatten)]
    resource_network: ResourceNetworkPlacementTables,
    starmap_placements: CatalogTable<StarmapPlacement>,
    facility_instances: CatalogTable<FacilityInstance>,
    container_instances: CatalogTable<ContainerInstance>,
}

impl<'source> Collected<'source> {
    pub(crate) fn catalog_placements(
        &mut self,
        forge: &DataForge<'_>,
        origins: &CatalogOrigins,
        container_instances: CatalogTable<ContainerInstance>,
        cancel: Option<&CancelToken>,
    ) -> Result<(CatalogPlacements, ResourceTopologyTables<'source>), CatalogError> {
        let topology = self.resource_topology(forge, &origins.uses, cancel)?;
        let starmap = std::mem::take(&mut self.starmap_objects);
        let placements = CatalogPlacements {
            resource_network: topology.placements,
            starmap_placements: starmap_placement::table(forge, starmap, origins)?,
            facility_instances: facility_loadout_placements::table(
                forge,
                &self.loadouts,
                &self.entity_configurations,
                origins,
                cancel,
            )?,
            container_instances,
        };
        Ok((placements, topology.declarations))
    }
}
