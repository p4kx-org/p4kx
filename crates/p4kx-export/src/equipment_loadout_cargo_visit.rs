// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Guid};

use super::read::Query;
use super::{LoadoutVisitSummary, scope, visit as loadout_visit};
use crate::CatalogError;
use crate::catalog_queries::{CatalogRecordCoverage, DatasetAvailability, ParsedCatalogQueries};
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::facility_instance::{FacilityConfiguration, FacilityInstance};
use crate::catalog_v3::facility_loadout_placements::project_loadout;
use crate::contract_loadout::Loadout;

pub(crate) struct ShipLoadoutPlacements<'source> {
    pub(crate) record_index: u32,
    pub(crate) guid: Guid,
    pub(crate) row: Option<&'source Loadout>,
    pub(crate) placements: &'source [FacilityInstance],
}

impl ParsedCatalogQueries<'_> {
    pub(crate) fn visit_ship_loadout_placements<E: From<CatalogError>>(
        &self,
        cancel: Option<&CancelToken>,
        configurations: impl FnMut(u32) -> Result<Vec<FacilityConfiguration>, CatalogError>,
        visit: impl FnMut(ShipLoadoutPlacements<'_>) -> Result<(), E>,
    ) -> Result<LoadoutVisitSummary, E> {
        self.project_fallible(cancel, |archive, entry, forge| {
            project(
                &Query {
                    archive,
                    entry,
                    forge,
                    cancel,
                },
                configurations,
                visit,
            )
        })
    }
}

fn project<E: From<CatalogError>>(
    query: &Query<'_>,
    mut configurations: impl FnMut(u32) -> Result<Vec<FacilityConfiguration>, CatalogError>,
    mut visit: impl FnMut(ShipLoadoutPlacements<'_>) -> Result<(), E>,
) -> Result<LoadoutVisitSummary, E> {
    let mut summary = LoadoutVisitSummary {
        build: query.archive.build().cloned(),
        availability: DatasetAvailability::Absent,
        owner_count: 0,
        row_count: 0,
        coverage: CatalogRecordCoverage::default(),
        missing_record_indices: Vec::new(),
        dependencies_complete: true,
    };
    let mut reads = CatalogRecordCoverage::default();
    let admission = loadout_visit::visit_ship_roots(query, |inputs, texts, want| {
        let ((owner, placements), _) = loadout_visit::with_resolved_owner(
            query,
            inputs,
            texts,
            want,
            &mut reads,
            |input, _, owner| {
                let placements = owner.row.as_ref().map_or_else(
                    || Ok(Vec::new()),
                    |row| project_loadout(row, input.origins(), query.cancel, &mut configurations),
                )?;
                Ok((owner, placements))
            },
        )?;
        checkpoint(query.cancel)?;
        visit(ShipLoadoutPlacements {
            record_index: want.record_index,
            guid: want.guid.get(),
            row: owner.row.as_ref(),
            placements: &placements,
        })?;
        summary.owner_count += 1;
        summary.row_count += usize::from(owner.row.is_some());
        summary.dependencies_complete &= owner.dependencies.is_complete();
        Ok::<_, E>(())
    })?;
    reads
        .unreadable_headers
        .extend(admission.unreadable_headers);
    reads.unreadable_bodies.extend(admission.unreadable_bodies);
    reads.incomplete_fields.extend(admission.incomplete_fields);
    reads.skipped_leftovers += admission.skipped_leftovers;
    scope::finish_coverage(&mut reads);
    summary.coverage = reads;
    summary.availability = query.availability(summary.row_count);
    Ok(summary)
}
