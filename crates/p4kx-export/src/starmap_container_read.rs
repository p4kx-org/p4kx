// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::time::Instant;

use p4kx_extraction::{Archive, CancelToken, Guid};

use super::jump_class::JumpEndpointRecords;
use super::name_coverage::PlacementNameCoverage;
use super::{Limits, PlacementScope, PlacementSpools, Placements};
use crate::catalog_read::original::EntryCapture;
use crate::catalog_table::DatasetAvailability;
use crate::export_ledger::ExportLedger;
use crate::harvest_locations::HarvestSurvey;
use crate::starmap_index::Index;
use crate::starmap_parse::{DescriptorScope, Read};
use crate::starmap_walk::{ObjectSelection, SystemRead};
use crate::{CatalogError, ExportAccountingError, Kind};

#[path = "starmap_container_systems.rs"]
mod systems;
use systems::SystemDescriptors;
pub(crate) use systems::unique_system_roots;

#[path = "starmap_shop_name_routes.rs"]
mod shop_name_routes;

#[derive(Clone, Copy)]
enum PlacementStorage<'directory> {
    Spool(&'directory Path),
    #[cfg(test)]
    Typed,
    Harvest(&'directory HarvestSurvey<'directory>),
    OpenedContainers,
    Object(ObjectSelection<'directory>),
    ShopNames(&'directory [Guid]),
    JumpEndpoints(Option<&'directory [Guid]>),
}

impl PlacementStorage<'_> {
    const fn covers_all_names(self) -> bool {
        match self {
            Self::Spool(_) | Self::ShopNames(_) => true,
            #[cfg(test)]
            Self::Typed => true,
            Self::Harvest(_)
            | Self::OpenedContainers
            | Self::Object(_)
            | Self::JumpEndpoints(_) => false,
        }
    }

    const fn descriptor_scope(self, jump_endpoints: bool) -> DescriptorScope {
        match self {
            Self::Harvest(_) => DescriptorScope::Harvest,
            Self::Object(ObjectSelection::Loot(_)) => DescriptorScope::LootPlacements,
            Self::OpenedContainers
            | Self::Object(ObjectSelection::One(_) | ObjectSelection::Records(_)) => {
                DescriptorScope::OpenedContainers
            }
            Self::ShopNames(_) => DescriptorScope::ShopNames,
            Self::Spool(_) => DescriptorScope::Catalog { jump_endpoints },
            Self::JumpEndpoints(_) => DescriptorScope::Catalog {
                jump_endpoints: true,
            },
            #[cfg(test)]
            Self::Typed => DescriptorScope::Catalog { jump_endpoints },
        }
    }
}

impl Placements {
    #[cfg(test)]
    pub(crate) fn read(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        capture: &EntryCapture<'_>,
        scope: PlacementScope,
    ) -> Result<Self, CatalogError> {
        Self::read_within(archive, cancel, Limits::default(), capture, scope)
    }

    pub(crate) fn read_for_catalog(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        limits: Limits,
        capture: &EntryCapture<'_>,
        scope: PlacementScope,
        endpoints: &JumpEndpointRecords,
        directory: &Path,
    ) -> Result<Self, CatalogError> {
        Self::read_scoped(
            archive,
            cancel,
            limits,
            capture,
            scope,
            Some(endpoints),
            PlacementStorage::Spool(directory),
        )
    }

    #[cfg(test)]
    pub(crate) fn read_within(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        limits: Limits,
        capture: &EntryCapture<'_>,
        scope: PlacementScope,
    ) -> Result<Self, CatalogError> {
        Self::read_scoped(
            archive,
            cancel,
            limits,
            capture,
            scope,
            None,
            PlacementStorage::Typed,
        )
    }

    pub(crate) fn read_for_harvest(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        limits: Limits,
        capture: &EntryCapture<'_>,
        harvest: &HarvestSurvey<'_>,
    ) -> Result<Self, CatalogError> {
        Self::read_scoped(
            archive,
            cancel,
            limits,
            capture,
            PlacementScope::RecordSelection,
            None,
            PlacementStorage::Harvest(harvest),
        )
    }

    pub(crate) fn read_loot_seeds(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        limits: Limits,
        capture: &EntryCapture<'_>,
        scope: PlacementScope,
    ) -> Result<Self, CatalogError> {
        match scope {
            PlacementScope::RecordSelection => {
                Self::read_for_objects(archive, cancel, limits, capture, &[])
            }
            PlacementScope::ArchiveInventory => Self::read_scoped(
                archive,
                cancel,
                limits,
                capture,
                scope,
                None,
                PlacementStorage::OpenedContainers,
            ),
        }
    }

    /// Retains the sorted requested Record placements and all physical Loot seeds.
    pub(crate) fn read_for_objects(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        limits: Limits,
        capture: &EntryCapture<'_>,
        guids: &[Guid],
    ) -> Result<Self, CatalogError> {
        if !guids.is_sorted() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Self::read_scoped(
            archive,
            cancel,
            limits,
            capture,
            PlacementScope::RecordSelection,
            None,
            PlacementStorage::Object(ObjectSelection::Loot(guids)),
        )
    }

    pub(crate) fn read_for_object(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        limits: Limits,
        capture: &EntryCapture<'_>,
        guid: Guid,
    ) -> Result<Self, CatalogError> {
        Self::read_scoped(
            archive,
            cancel,
            limits,
            capture,
            PlacementScope::ArchiveInventory,
            None,
            PlacementStorage::Object(ObjectSelection::One(guid)),
        )
    }

    pub(crate) fn read_for_jump_endpoints(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        limits: Limits,
        capture: &EntryCapture<'_>,
        scope: PlacementScope,
        endpoints: &JumpEndpointRecords,
        records: Option<&[Guid]>,
    ) -> Result<Self, CatalogError> {
        if records.is_some_and(|guids| !guids.is_sorted()) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Self::read_scoped(
            archive,
            cancel,
            limits,
            capture,
            scope,
            Some(endpoints),
            PlacementStorage::JumpEndpoints(records),
        )
    }

    fn empty_inventory(
        archive: &Archive,
        capture: &EntryCapture<'_>,
        endpoints: Option<&JumpEndpointRecords>,
        directory_complete: bool,
        storage: PlacementStorage<'_>,
    ) -> Result<Self, CatalogError> {
        if endpoints.is_some() {
            capture
                .source_uses()
                .observe_root_directory_paths(archive)?;
        }
        Ok(Self {
            instance_availability: Some(DatasetAvailability::Absent),
            spools: PlacementSpools {
                location_physics_availability: Some(crate::location_physics::stage::availability(
                    directory_complete,
                    false,
                )),
                ..PlacementSpools::default()
            },
            endpoint_inventory_complete: directory_complete
                && endpoints.is_some_and(|records| records.complete),
            name_coverage: PlacementNameCoverage {
                directory_complete,
                descriptors_complete: storage.covers_all_names(),
                routes_complete: storage.covers_all_names(),
            },
            ..Self::default()
        })
    }

    fn read_scoped(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        limits: Limits,
        capture: &EntryCapture<'_>,
        scope: PlacementScope,
        endpoints: Option<&JumpEndpointRecords>,
        storage: PlacementStorage<'_>,
    ) -> Result<Self, CatalogError> {
        check_cancelled(cancel)?;
        let started = Instant::now();
        let index = Index::new(archive);
        let systems = index.systems();
        let directory_complete = grid_directory_complete(archive);
        if systems.is_empty() && matches!(scope, PlacementScope::ArchiveInventory) {
            return Self::empty_inventory(archive, capture, endpoints, directory_complete, storage);
        }
        let bounds = (
            limits.source_children,
            storage.descriptor_scope(endpoints.is_some()),
        );
        let SystemDescriptors {
            systems,
            system_count,
            all_roots,
            names_complete,
            read:
                Read {
                    bodies,
                    mut ledger,
                    survey,
                    ..
                },
        } = systems::read_descriptors(archive, &index, systems, cancel, bounds, capture)?;
        check_cancelled(cancel)?;
        let (read, mut spools) = systems::walk_systems(
            &index,
            &bodies,
            &systems,
            cancel,
            limits,
            (archive, capture, endpoints),
            storage,
        )?;
        drop(bodies);
        check_cancelled(cancel)?;
        let routes_complete = finish_route_coverage(&read, &mut ledger);
        spools.finish_grid_scope(storage, archive, routes_complete && all_roots, &ledger);
        let mut placements = Self {
            ledger,
            spools,
            name_coverage: PlacementNameCoverage {
                directory_complete,
                descriptors_complete: names_complete,
                routes_complete: storage.covers_all_names() && routes_complete,
            },
            ..Self::default()
        };
        placements.finish(&index, system_count, read, scope, (archive, capture))?;
        placements.finish_scope_coverage(capture, endpoints, routes_complete, survey);
        placements.report.elapsed = started.elapsed();
        Ok(placements)
    }

    fn has_complete_endpoints(
        &self,
        routes_complete: bool,
        endpoints: Option<&JumpEndpointRecords>,
    ) -> bool {
        routes_complete
            && !self
                .ledger
                .incomplete_inventories
                .contains(&Kind::StarmapObjects)
            && endpoints.is_some_and(|records| records.complete)
            && self.counts.unreadable == 0
            && self.counts.missing == 0
    }
}

impl PlacementSpools {
    fn finish_grid_scope(
        &mut self,
        storage: PlacementStorage<'_>,
        archive: &Archive,
        routes_complete: bool,
        ledger: &ExportLedger,
    ) {
        if !matches!(storage, PlacementStorage::Spool(_)) {
            return;
        }
        let readable = routes_complete
            && grid_directory_complete(archive)
            && !ledger
                .incomplete_inventories
                .contains(&Kind::StarmapObjects);
        if matches!(
            self.location_physics_availability,
            Some(DatasetAvailability::Available)
        ) {
            return;
        }
        if !readable {
            self.location_physics_availability = Some(DatasetAvailability::Unavailable);
        } else if self.location_physics_availability.is_none() {
            self.location_physics_availability = Some(DatasetAvailability::Absent);
        }
    }
}

fn grid_directory_complete(archive: &Archive) -> bool {
    crate::export_archive_health::inventory(archive).is_none()
}

pub(crate) fn finish_route_coverage(read: &[SystemRead], ledger: &mut ExportLedger) -> bool {
    if read.iter().any(|system| system.placement_budget_exhausted) {
        ledger.incomplete_inventory(Kind::StarmapObjects);
    }
    read.iter().all(|system| !system.routes_incomplete)
}

fn check_cancelled(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    Ok(())
}
