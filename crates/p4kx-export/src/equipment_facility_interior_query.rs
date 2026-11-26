// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForgeEntry, SourceInventory, SourceValueState,
};

use super::{FacilityInstance, LoadoutDependencyCoverage, read::Query};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, DatasetAvailability,
    ParsedCatalogQueries,
};
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::facility_loadout_placements::facility_placement_source_type;

#[path = "equipment_facility_interior_read.rs"]
mod read_scope;
#[path = "equipment_facility_interior_visit.rs"]
mod visit;

#[path = "equipment_facility_interior_sources.rs"]
mod sources;

pub use crate::catalog_v3::facility_loadout_placements::{
    InteriorFacilityPlacement, InteriorRoomCoverage, InteriorSocPosition, SocAttribute,
};
use read_scope::selected;
pub use sources::InteriorCoverageDependency;
pub use visit::{
    FacilityInteriorPlacementVisitSummary, visit_facility_interior_placements,
    visit_facility_interior_placements_from,
};

/// Original room coverage reached from one physical Ship.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct FacilityInteriorCoverageSnapshot {
    /// Build of the queried Archive.
    pub build: Option<Build>,
    /// Physical Record selector supplied to the query.
    pub ship_record_index: u32,
    /// Whether this physical root belongs to the canonical Ship selection.
    pub root_state: SourceValueState,
    /// Whether a Loadout could supply additional attachment paths.
    pub loadout_state: SourceValueState,
    /// Direct room declarations followed by attached rooms in original port order.
    pub rows: Vec<InteriorRoomCoverage>,
    /// Whether the selected scope and original room traversals were complete.
    pub complete: bool,
    /// Original Ship admission and Loadout coverage.
    pub loadout_coverage: CatalogRecordCoverage,
    /// Resolver and alignment coverage of the selected Loadout.
    pub loadout_dependencies: LoadoutDependencyCoverage,
    /// Physical Record dependencies of declaration and class selection.
    pub dependencies: Vec<InteriorCoverageDependency>,
    /// One source inventory for the selected scope.
    pub source_inventory: SourceInventory,
}

/// Interior facility placements reached from one physical Ship.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct FacilityInteriorPlacementSnapshot {
    /// Availability of placement source declarations in the Archive.
    pub availability: DatasetAvailability,
    /// Interior instances in direct room and attached port order.
    pub rows: Vec<FacilityInstance>,
    /// Coverage of the selected room traversals and source inventory for both row sets.
    pub coverage: FacilityInteriorCoverageSnapshot,
}

/// Reads interior room coverage for one physical Ship.
///
/// # Errors
/// Returns input, source evidence, Archive-change or cancellation errors.
pub fn read_facility_interior_coverage(
    archive: &Archive,
    ship_record_index: u32,
    cancel: Option<&CancelToken>,
) -> Result<FacilityInteriorCoverageSnapshot, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .read_facility_interior_coverage(ship_record_index, cancel)
}

/// Reads interior room coverage using decoded DataForge bytes from the same Archive.
///
/// # Errors
/// Returns query errors or an input-origin mismatch.
pub fn read_facility_interior_coverage_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    ship_record_index: u32,
    cancel: Option<&CancelToken>,
) -> Result<FacilityInteriorCoverageSnapshot, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .read_facility_interior_coverage(ship_record_index, cancel)
}

/// Reads Interior-kind facility placements for one physical Ship.
///
/// # Errors
/// Returns input, source evidence, Archive-change or cancellation errors.
pub fn read_facility_interior_placements(
    archive: &Archive,
    ship_record_index: u32,
    cancel: Option<&CancelToken>,
) -> Result<FacilityInteriorPlacementSnapshot, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .read_facility_interior_placements(ship_record_index, cancel)
}

/// Reads interior placements using decoded DataForge bytes from the same Archive.
///
/// # Errors
/// Returns query errors or an input-origin mismatch.
pub fn read_facility_interior_placements_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    ship_record_index: u32,
    cancel: Option<&CancelToken>,
) -> Result<FacilityInteriorPlacementSnapshot, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .read_facility_interior_placements(ship_record_index, cancel)
}

impl CatalogQuerySession<'_> {
    /// Reads interior room coverage through the current session.
    ///
    /// # Errors
    /// Returns input, source evidence, Archive-change or cancellation errors.
    pub fn read_facility_interior_coverage(
        &self,
        ship_record_index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<FacilityInteriorCoverageSnapshot, CatalogError> {
        self.parsed(cancel)?
            .read_facility_interior_coverage(ship_record_index, cancel)
    }

    /// Reads interior placements through the current session.
    ///
    /// # Errors
    /// Returns input, source evidence, Archive-change or cancellation errors.
    pub fn read_facility_interior_placements(
        &self,
        ship_record_index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<FacilityInteriorPlacementSnapshot, CatalogError> {
        self.parsed(cancel)?
            .read_facility_interior_placements(ship_record_index, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads interior room coverage through the parsed DataForge.
    ///
    /// # Errors
    /// Returns input, source evidence, Archive-change or cancellation errors.
    pub fn read_facility_interior_coverage(
        &self,
        ship_record_index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<FacilityInteriorCoverageSnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            selected(
                &Query {
                    archive,
                    entry,
                    forge,
                    cancel,
                },
                ship_record_index,
                false,
            )
            .map(|(coverage, _)| coverage)
        })
    }

    /// Reads interior placements through the parsed DataForge.
    ///
    /// # Errors
    /// Returns input, source evidence, Archive-change or cancellation errors.
    pub fn read_facility_interior_placements(
        &self,
        ship_record_index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<FacilityInteriorPlacementSnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            let (coverage, rows) = selected(
                &Query {
                    archive,
                    entry,
                    forge,
                    cancel,
                },
                ship_record_index,
                true,
            )?;
            let table =
                CatalogTable::matching_source_types(forge, facility_placement_source_type, rows);
            Ok(FacilityInteriorPlacementSnapshot {
                availability: table.availability,
                rows: table.rows,
                coverage,
            })
        })
    }
}
