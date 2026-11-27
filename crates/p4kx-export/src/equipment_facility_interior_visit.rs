// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Build, CancelToken, DataForgeEntry, PathKeys, SourceValueState};

use super::super::read::{Query, ResolverInputs};
use super::super::visit;
use super::FacilityInteriorPlacementSnapshot;
use super::read_scope::{ResolvedLoadout, finish_scope, read_room_scope};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, DatasetAvailability,
    ParsedCatalogQueries,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::facility_loadout_placements::facility_placement_source_type;
use crate::loadout::Wanted;
use crate::texts::Texts;

/// Terminal counts and original Ship admission coverage of an Interior placement visit.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct FacilityInteriorPlacementVisitSummary {
    /// Build of the queried Archive.
    pub build: Option<Build>,
    /// Availability of placement source declarations independently of the row counts.
    pub availability: DatasetAvailability,
    /// Number of admitted physical Ship owners visited, including empty or incomplete owners.
    pub owner_count: usize,
    /// Number of Interior facility placements emitted by all owners.
    pub placement_count: usize,
    /// Number of original room coverage rows emitted by all owners.
    pub coverage_count: usize,
    /// Number of visited Ship owners whose Loadout or room scope reported incomplete coverage.
    pub incomplete_owner_count: usize,
    /// Coverage of the physical Ship admission scan.
    pub ship_coverage: CatalogRecordCoverage,
}

impl FacilityInteriorPlacementVisitSummary {
    fn record(&mut self, snapshot: &FacilityInteriorPlacementSnapshot) {
        self.owner_count += 1;
        self.placement_count += snapshot.rows.len();
        self.coverage_count += snapshot.coverage.rows.len();
        self.incomplete_owner_count += usize::from(!snapshot.coverage.complete);
        if !snapshot.rows.is_empty() {
            self.availability = DatasetAvailability::Available;
        }
    }
}

/// Visits all admitted physical Ships in canonical Loadout owner order.
///
/// # Errors
/// Returns query, cancellation, Archive-change or callback errors.
pub fn visit_facility_interior_placements<E: From<CatalogError>>(
    archive: &Archive,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(&FacilityInteriorPlacementSnapshot) -> Result<(), E>,
) -> Result<FacilityInteriorPlacementVisitSummary, E> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .visit_facility_interior_placements(cancel, visit)
}

/// Visits Interior placements using decoded DataForge bytes from the same Archive.
///
/// # Errors
/// Returns query or callback errors, including a foreign Entry origin.
pub fn visit_facility_interior_placements_from<E: From<CatalogError>>(
    archive: &Archive,
    entry: &DataForgeEntry,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(&FacilityInteriorPlacementSnapshot) -> Result<(), E>,
) -> Result<FacilityInteriorPlacementVisitSummary, E> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .visit_facility_interior_placements(cancel, visit)
}

impl CatalogQuerySession<'_> {
    /// Visits Interior placements through this reusable Archive session.
    ///
    /// # Errors
    /// Returns query, cancellation, Archive-change or callback errors.
    pub fn visit_facility_interior_placements<E: From<CatalogError>>(
        &self,
        cancel: Option<&CancelToken>,
        visit: impl FnMut(&FacilityInteriorPlacementSnapshot) -> Result<(), E>,
    ) -> Result<FacilityInteriorPlacementVisitSummary, E> {
        self.parsed(cancel)?
            .visit_facility_interior_placements(cancel, visit)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Visits Interior placements through this parsed DataForge.
    ///
    /// Each callback receives one physical Ship, including empty and incomplete owners.
    ///
    /// # Errors
    /// Returns query, cancellation, Archive-change or callback errors.
    pub fn visit_facility_interior_placements<E: From<CatalogError>>(
        &self,
        cancel: Option<&CancelToken>,
        visit: impl FnMut(&FacilityInteriorPlacementSnapshot) -> Result<(), E>,
    ) -> Result<FacilityInteriorPlacementVisitSummary, E> {
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
    mut visit: impl FnMut(&FacilityInteriorPlacementSnapshot) -> Result<(), E>,
) -> Result<FacilityInteriorPlacementVisitSummary, E> {
    let declared =
        DatasetAvailability::from_source_declarations(query.forge, facility_placement_source_type);
    let mut summary = FacilityInteriorPlacementVisitSummary {
        build: query.archive.build().cloned(),
        availability: declared,
        owner_count: 0,
        placement_count: 0,
        coverage_count: 0,
        incomplete_owner_count: 0,
        ship_coverage: CatalogRecordCoverage::default(),
    };
    let archive_complete = crate::export_archive_health::inventory(query.archive).is_none();
    let mut path_keys = None;
    summary.ship_coverage = visit::visit_ship_roots(query, |inputs, texts, want| {
        let snapshot = read_owner(
            query,
            inputs,
            texts,
            want,
            declared,
            archive_complete,
            &mut path_keys,
        )?;
        checkpoint(query.cancel)?;
        visit(&snapshot)?;
        summary.record(&snapshot);
        Ok::<(), E>(())
    })?;
    Ok(summary)
}

fn read_owner(
    query: &Query<'_>,
    inputs: &ResolverInputs<'_>,
    texts: &Texts<'_>,
    want: Wanted,
    declared: DatasetAvailability,
    archive_complete: bool,
    path_keys: &mut Option<PathKeys>,
) -> Result<FacilityInteriorPlacementSnapshot, CatalogError> {
    let mut coverage = CatalogRecordCoverage::default();
    let ((owner, rooms), source_inventory) = visit::with_resolved_owner(
        query,
        inputs,
        texts,
        want,
        &mut coverage,
        |input, alignment, owner| {
            let rooms = read_room_scope(
                query,
                input,
                Some(want),
                owner.row.as_ref(),
                alignment,
                true,
                path_keys,
            )?;
            Ok((owner, rooms))
        },
    )?;
    let loadout = ResolvedLoadout::new(Some(want), SourceValueState::Present, owner, coverage);
    let (coverage, rows) = finish_scope(
        query,
        archive_complete,
        want.record_index,
        loadout,
        rooms,
        source_inventory,
    );
    let availability = if rows.is_empty() {
        declared
    } else {
        DatasetAvailability::Available
    };
    Ok(FacilityInteriorPlacementSnapshot {
        availability,
        rows,
        coverage,
    })
}
