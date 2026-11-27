// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Build, CancelToken, DataForgeEntry, SourceValueState};

use super::super::read::{Query, ResolverInputs};
use super::super::{scope, visit as loadout_visit};
use super::{InstalledConfigurationSnapshot, ProjectedConfigurations, read_placements};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, DatasetAvailability,
    ParsedCatalogQueries,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::facility_loadout_placements::scoped_table;
use crate::loadout::Wanted;
use crate::texts::Texts;

/// Terminal counts and Ship admission coverage for an installed flight configuration visit.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct InstalledConfigurationVisitSummary {
    /// Build of the queried Archive.
    pub build: Option<Build>,
    /// Availability of the original flight component families.
    pub availability: DatasetAvailability,
    /// Number of admitted physical Ships visited.
    pub owner_count: usize,
    /// Number of installed configuration rows emitted by the visit.
    pub placement_count: usize,
    /// Number of Ships whose Loadout or required configuration reads were incomplete.
    pub incomplete_owner_count: usize,
    /// Coverage of the physical Ship admission scan.
    pub ship_coverage: CatalogRecordCoverage,
}

/// Visits installed flight configurations in canonical Ship Loadout order.
/// Each callback receives one physical Ship, including empty and unreadable owners.
///
/// # Errors
/// Returns query, cancellation, Archive-change or callback errors.
pub fn visit_installed_configurations<E: From<CatalogError>>(
    archive: &Archive,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(&InstalledConfigurationSnapshot) -> Result<(), E>,
) -> Result<InstalledConfigurationVisitSummary, E> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .visit_installed_configurations(cancel, visit)
}

/// Visits installed flight configurations with decoded DataForge bytes from the same Archive.
///
/// # Errors
/// Returns query or callback errors, including a foreign Entry origin.
pub fn visit_installed_configurations_from<E: From<CatalogError>>(
    archive: &Archive,
    entry: &DataForgeEntry,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(&InstalledConfigurationSnapshot) -> Result<(), E>,
) -> Result<InstalledConfigurationVisitSummary, E> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .visit_installed_configurations(cancel, visit)
}

impl CatalogQuerySession<'_> {
    /// Visits installed flight configurations through this reusable Archive session.
    ///
    /// # Errors
    /// Returns query, cancellation, Archive-change or callback errors.
    pub fn visit_installed_configurations<E: From<CatalogError>>(
        &self,
        cancel: Option<&CancelToken>,
        visit: impl FnMut(&InstalledConfigurationSnapshot) -> Result<(), E>,
    ) -> Result<InstalledConfigurationVisitSummary, E> {
        self.parsed(cancel)?
            .visit_installed_configurations(cancel, visit)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Visits installed flight configurations through this parsed DataForge.
    /// Inspect terminal coverage before committing callback output.
    ///
    /// # Errors
    /// Returns query, cancellation, Archive-change or callback errors.
    pub fn visit_installed_configurations<E: From<CatalogError>>(
        &self,
        cancel: Option<&CancelToken>,
        visit: impl FnMut(&InstalledConfigurationSnapshot) -> Result<(), E>,
    ) -> Result<InstalledConfigurationVisitSummary, E> {
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
    mut visit: impl FnMut(&InstalledConfigurationSnapshot) -> Result<(), E>,
) -> Result<InstalledConfigurationVisitSummary, E> {
    let declared = scoped_table(query.forge, Vec::new()).availability;
    let mut summary = InstalledConfigurationVisitSummary {
        build: query.archive.build().cloned(),
        availability: declared,
        owner_count: 0,
        placement_count: 0,
        incomplete_owner_count: 0,
        ship_coverage: CatalogRecordCoverage::default(),
    };
    summary.ship_coverage = loadout_visit::visit_ship_roots(query, |inputs, texts, want| {
        let snapshot = read_owner(query, inputs, texts, want, declared)?;
        checkpoint(query.cancel)?;
        visit(&snapshot)?;
        summary.owner_count += 1;
        summary.placement_count += snapshot.rows.len();
        summary.incomplete_owner_count += usize::from(!snapshot.is_complete());
        if !snapshot.rows.is_empty() {
            summary.availability = DatasetAvailability::Available;
        }
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
) -> Result<InstalledConfigurationSnapshot, CatalogError> {
    let mut loadout_coverage = CatalogRecordCoverage::default();
    let ((loadout_state, loadout_dependencies, configurations), source_inventory) =
        loadout_visit::with_resolved_owner(
            query,
            inputs,
            texts,
            want,
            &mut loadout_coverage,
            |input, _, owner| {
                let (state, configurations) = match owner.row.as_ref() {
                    Some(row) => (
                        SourceValueState::Present,
                        read_placements(query, input, row)?,
                    ),
                    None => (
                        SourceValueState::Unreadable,
                        ProjectedConfigurations::default(),
                    ),
                };
                Ok((state, owner.dependencies, configurations))
            },
        )?;
    let availability = if configurations.rows.is_empty() {
        declared
    } else {
        DatasetAvailability::Available
    };
    scope::finish_coverage(&mut loadout_coverage);
    Ok(InstalledConfigurationSnapshot {
        build: query.archive.build().cloned(),
        ship_record_index: want.record_index,
        availability,
        loadout_state,
        rows: configurations.rows,
        source_inventory,
        loadout_coverage,
        loadout_dependencies,
        configuration_dependencies: configurations.dependencies,
    })
}
