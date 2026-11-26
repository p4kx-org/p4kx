// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForgeEntry, SourceInventory, SourceValueState,
};

use super::read::Query;
use super::{LoadoutDependencyCoverage, scope};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, DatasetAvailability,
    ParsedCatalogQueries,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::catalog_entity_configurations::{
    loadout_facility_owner_source_type, read_loadout_facility_owners,
};
use crate::catalog_v3::facility_instance::FacilityConfiguration;
use crate::catalog_v3::facility_loadout_placements::{
    loadout_configuration_links, project_loadout,
};
use crate::contract_loadout::Loadout;
use crate::loadout::alignment::direct::{AlignmentDependencyState, DirectAlignment};
use crate::texts::{TextSources, Texts};
use crate::{ExportAccountingError, Kind};

pub use crate::catalog_v3::facility_instance::{FacilityConfigurationKind, FacilityInstance};

#[path = "equipment_loadout_facility_visit.rs"]
mod visit;
pub use visit::{
    LoadoutFacilityPlacementVisitSummary, visit_loadout_facility_placements,
    visit_loadout_facility_placements_from,
};

/// An original Record consulted for Facility or Inventory storage links.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct FacilityPlacementDependency {
    /// The physical Record index in the queried DataForge.
    pub record_index: u32,
    /// The outcome of classifying and reading that Record's configuration links.
    pub state: AlignmentDependencyState,
}

/// Loadout-kind facility placements on one physical Ship Record.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct LoadoutFacilityPlacementSnapshot {
    /// The Build of the original Archive.
    pub build: Option<Build>,
    /// Physical index of the queried Ship in the original DataForge.
    pub ship_record_index: u32,
    /// Availability of the original Facility and Inventory source families.
    pub availability: DatasetAvailability,
    /// Whether the queried Loadout is present, absent or unreadable.
    pub loadout_state: SourceValueState,
    /// Resolved instances in Loadout traversal order.
    pub rows: Vec<FacilityInstance>,
    /// Original sources and uses of the result.
    pub source_inventory: SourceInventory,
    /// Read coverage of the selected Ship and its Loadout.
    pub loadout_coverage: CatalogRecordCoverage,
    /// Resolver and alignment dependencies of the selected Loadout.
    pub loadout_dependencies: LoadoutDependencyCoverage,
    /// Reached configuration dependencies in physical Record order.
    pub configuration_dependencies: Vec<FacilityPlacementDependency>,
}

impl LoadoutFacilityPlacementSnapshot {
    /// Reports whether the original Loadout and reached configuration reads were conclusive.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.loadout_state != SourceValueState::Unreadable
            && self.loadout_coverage.unreadable_headers.is_empty()
            && self.loadout_coverage.unreadable_bodies.is_empty()
            && self.loadout_coverage.incomplete_fields.is_empty()
            && self.loadout_dependencies.is_complete()
            && self.configuration_dependencies.iter().all(|dependency| {
                matches!(
                    dependency.state,
                    AlignmentDependencyState::Complete | AlignmentDependencyState::NotAdmitted
                )
            })
    }
}

/// Reads Loadout-kind facility placements from one physical Ship Record.
///
/// # Errors
/// Returns input, source evidence, Archive-change or cancellation errors.
pub fn read_loadout_facility_placements(
    archive: &Archive,
    ship_record_index: u32,
    cancel: Option<&CancelToken>,
) -> Result<LoadoutFacilityPlacementSnapshot, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .read_loadout_facility_placements(ship_record_index, cancel)
}

/// Reads Loadout-kind facility placements using an already decoded Archive Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn read_loadout_facility_placements_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    ship_record_index: u32,
    cancel: Option<&CancelToken>,
) -> Result<LoadoutFacilityPlacementSnapshot, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .read_loadout_facility_placements(ship_record_index, cancel)
}

impl CatalogQuerySession<'_> {
    /// Reads one ship's Loadout-kind facility placements through this session.
    ///
    /// # Errors
    /// Returns input, source evidence, Archive-change or cancellation errors.
    pub fn read_loadout_facility_placements(
        &self,
        ship_record_index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<LoadoutFacilityPlacementSnapshot, CatalogError> {
        self.parsed(cancel)?
            .read_loadout_facility_placements(ship_record_index, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads one ship's Loadout-kind facility placements through this parsed DataForge.
    ///
    /// # Errors
    /// Returns input, source evidence, Archive-change or cancellation errors.
    pub fn read_loadout_facility_placements(
        &self,
        ship_record_index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<LoadoutFacilityPlacementSnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            selected(
                &Query {
                    archive,
                    entry,
                    forge,
                    cancel,
                },
                ship_record_index,
            )
        })
    }
}

fn selected(
    query: &Query<'_>,
    ship_record_index: u32,
) -> Result<LoadoutFacilityPlacementSnapshot, CatalogError> {
    let scope::Scope {
        wanted,
        coverage: mut loadout_coverage,
    } = scope::read_ship(query.forge, ship_record_index, query.cancel)?;
    let admitted = !wanted.is_empty();
    let origins = query.entry.records(query.archive)?;
    let input = OriginalCatalogInput::new(query.forge, &origins);
    let locale = TextSources::read_languages(query.archive, &[])?;
    let texts = Texts::parse(&locale)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(Kind::Loadouts);
    let mut alignment = DirectAlignment::new(&projection, query.cancel);
    let (loadouts, loadout_dependencies) =
        query.resolve_rows(&input, wanted, &mut loadout_coverage, &mut alignment)?;
    let (loadout_state, placements) = match loadouts.as_slice() {
        [loadout] => (
            SourceValueState::Present,
            read_placements(loadout, &origins, &mut alignment, query.cancel)?,
        ),
        [] if admitted || !loadout_coverage.unreadable_headers.is_empty() => {
            (SourceValueState::Unreadable, ProjectedPlacements::default())
        }
        [] => (SourceValueState::Absent, ProjectedPlacements::default()),
        _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
    };
    scope::finish_coverage(&mut loadout_coverage);
    let availability = DatasetAvailability::from_source_declarations(
        query.forge,
        loadout_facility_owner_source_type,
    );
    let source_inventory = origins.uses.finish_with(|| checkpoint(query.cancel))?;
    checkpoint(query.cancel)?;
    Ok(LoadoutFacilityPlacementSnapshot {
        build: query.archive.build().cloned(),
        ship_record_index,
        availability,
        loadout_state,
        rows: placements.rows,
        source_inventory,
        loadout_coverage,
        loadout_dependencies,
        configuration_dependencies: placements.dependencies,
    })
}

pub(super) struct ResolvedConfiguration {
    pub(super) state: AlignmentDependencyState,
    pub(super) links: Vec<FacilityConfiguration>,
}

#[derive(Default)]
struct ProjectedPlacements {
    rows: Vec<FacilityInstance>,
    dependencies: Vec<FacilityPlacementDependency>,
}

fn read_placements(
    loadout: &Loadout,
    origins: &CatalogOrigins,
    alignment: &mut DirectAlignment<'_>,
    cancel: Option<&CancelToken>,
) -> Result<ProjectedPlacements, CatalogError> {
    let mut configurations = BTreeMap::new();
    let rows = project_loadout(loadout, origins, cancel, &mut |index| {
        let configuration = match configurations.entry(index) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(read_configuration(alignment, index)?),
        };
        Ok(configuration.links.clone())
    })?;
    let dependencies = configurations
        .into_iter()
        .map(|(record_index, resolved)| FacilityPlacementDependency {
            record_index,
            state: resolved.state,
        })
        .collect();
    Ok(ProjectedPlacements { rows, dependencies })
}

pub(super) fn read_configuration(
    alignment: &mut DirectAlignment<'_>,
    index: u32,
) -> Result<ResolvedConfiguration, CatalogError> {
    let read = alignment.read_configuration(index, |tree, projection, admission, cancel| {
        let read = read_loadout_facility_owners(tree, projection, admission, cancel)?;
        Ok(ResolvedConfiguration {
            state: AlignmentDependencyState::from_scoped_read(&read),
            links: loadout_configuration_links(index, &read.value, cancel)?,
        })
    })?;
    Ok(read.unwrap_or_else(|state| ResolvedConfiguration {
        state,
        links: Vec::new(),
    }))
}
