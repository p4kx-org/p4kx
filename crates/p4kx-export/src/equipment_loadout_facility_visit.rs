// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Build, CancelToken, DataForgeEntry, SourceValueState};

use super::super::read::{Query, ResolverInputs};
use super::super::{scope, visit as loadout_visit};
use super::{LoadoutFacilityPlacementSnapshot, ProjectedPlacements, read_placements};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, DatasetAvailability,
    ParsedCatalogQueries,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::catalog_entity_configurations::loadout_facility_owner_source_type;
use crate::loadout::Wanted;
use crate::texts::Texts;

/// Terminal counts and original Ship admission coverage of a Loadout facility placement visit.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct LoadoutFacilityPlacementVisitSummary {
    /// Build of the queried Archive.
    pub build: Option<Build>,
    /// Availability of the original Facility and Inventory source families.
    pub availability: DatasetAvailability,
    /// Number of admitted physical Ship owners visited, including empty or unreadable owners.
    pub owner_count: usize,
    /// Number of Loadout placement rows emitted by all owners.
    pub placement_count: usize,
    /// Number of visited owners with an unreadable Loadout or incomplete dependencies.
    pub incomplete_owner_count: usize,
    /// Coverage of the physical Ship admission scan.
    pub ship_coverage: CatalogRecordCoverage,
}

/// Visits Loadout facility placements for every admitted Ship in canonical owner order.
/// Inspect terminal coverage before committing consumer-owned output.
///
/// # Errors
/// Returns query, cancellation, Archive-change or callback errors.
pub fn visit_loadout_facility_placements<E: From<CatalogError>>(
    archive: &Archive,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(&LoadoutFacilityPlacementSnapshot) -> Result<(), E>,
) -> Result<LoadoutFacilityPlacementVisitSummary, E> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .visit_loadout_facility_placements(cancel, visit)
}

/// Visits Loadout facility placements with decoded DataForge bytes from the same Archive.
///
/// # Errors
/// Returns query or callback errors, including a foreign Entry origin.
pub fn visit_loadout_facility_placements_from<E: From<CatalogError>>(
    archive: &Archive,
    entry: &DataForgeEntry,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(&LoadoutFacilityPlacementSnapshot) -> Result<(), E>,
) -> Result<LoadoutFacilityPlacementVisitSummary, E> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .visit_loadout_facility_placements(cancel, visit)
}

impl CatalogQuerySession<'_> {
    /// Visits Loadout facility placements through this reusable Archive session.
    ///
    /// # Errors
    /// Returns query, cancellation, Archive-change or callback errors.
    pub fn visit_loadout_facility_placements<E: From<CatalogError>>(
        &self,
        cancel: Option<&CancelToken>,
        visit: impl FnMut(&LoadoutFacilityPlacementSnapshot) -> Result<(), E>,
    ) -> Result<LoadoutFacilityPlacementVisitSummary, E> {
        self.parsed(cancel)?
            .visit_loadout_facility_placements(cancel, visit)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Visits Loadout facility placements through this parsed DataForge.
    /// The callback receives each physical Ship, even when it yields no placements.
    ///
    /// # Errors
    /// Returns query, cancellation, Archive-change or callback errors.
    pub fn visit_loadout_facility_placements<E: From<CatalogError>>(
        &self,
        cancel: Option<&CancelToken>,
        visit: impl FnMut(&LoadoutFacilityPlacementSnapshot) -> Result<(), E>,
    ) -> Result<LoadoutFacilityPlacementVisitSummary, E> {
        self.project_fallible(cancel, |archive, entry, forge| {
            visit_all(
                &Query {
                    archive,
                    entry,
                    forge,
                    cancel,
                },
                visit,
            )
        })
    }
}

fn visit_all<E: From<CatalogError>>(
    query: &Query<'_>,
    mut visit: impl FnMut(&LoadoutFacilityPlacementSnapshot) -> Result<(), E>,
) -> Result<LoadoutFacilityPlacementVisitSummary, E> {
    let availability = DatasetAvailability::from_source_declarations(
        query.forge,
        loadout_facility_owner_source_type,
    );
    let mut summary = LoadoutFacilityPlacementVisitSummary {
        build: query.archive.build().cloned(),
        availability,
        owner_count: 0,
        placement_count: 0,
        incomplete_owner_count: 0,
        ship_coverage: CatalogRecordCoverage::default(),
    };
    summary.ship_coverage = loadout_visit::visit_ship_roots(query, |inputs, texts, want| {
        let snapshot = read_owner(query, inputs, texts, want, availability)?;
        checkpoint(query.cancel)?;
        visit(&snapshot)?;
        summary.owner_count += 1;
        summary.placement_count += snapshot.rows.len();
        summary.incomplete_owner_count += usize::from(!snapshot.is_complete());
        Ok::<(), E>(())
    })?;
    Ok(summary)
}

fn read_owner(
    query: &Query<'_>,
    inputs: &ResolverInputs<'_>,
    texts: &Texts<'_>,
    want: Wanted,
    availability: DatasetAvailability,
) -> Result<LoadoutFacilityPlacementSnapshot, CatalogError> {
    let mut loadout_coverage = CatalogRecordCoverage::default();
    let ((loadout_state, loadout_dependencies, placements), source_inventory) =
        loadout_visit::with_resolved_owner(
            query,
            inputs,
            texts,
            want,
            &mut loadout_coverage,
            |input, alignment, owner| {
                let (state, placements) = match owner.row.as_ref() {
                    Some(row) => (
                        SourceValueState::Present,
                        read_placements(row, input.origins(), alignment, query.cancel)?,
                    ),
                    None => (SourceValueState::Unreadable, ProjectedPlacements::default()),
                };
                Ok((state, owner.dependencies, placements))
            },
        )?;
    scope::finish_coverage(&mut loadout_coverage);
    Ok(LoadoutFacilityPlacementSnapshot {
        build: query.archive.build().cloned(),
        ship_record_index: want.record_index,
        availability,
        loadout_state,
        rows: placements.rows,
        source_inventory,
        loadout_coverage,
        loadout_dependencies,
        configuration_dependencies: placements.dependencies,
    })
}
