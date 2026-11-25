// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Owner-scoped visits of canonical Loadout rows.

use p4kx_extraction::{Archive, Build, CancelToken, DataForgeEntry, Guid, SourceInventory};

use super::read::{Query, ResolverInputs};
use super::{Loadout, LoadoutDependencyCoverage, LoadoutOwnerKind, LoadoutOwnerSelection, scope};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries,
};
use crate::catalog_query_session::checkpoint;
use crate::loadout::Wanted;
use crate::loadout::alignment::direct::DirectAlignment;
use crate::loadout::resolution;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, ExportAccountingError, Kind};

/// One selected owner, its canonical row when emitted, and original dependency evidence.
#[derive(Debug)]
#[non_exhaustive]
pub struct LoadoutOwnerSnapshot {
    /// Physical DataForge Record position.
    pub record_index: u32,
    /// Original GUID, which may have other physical occurrences.
    pub entity_guid: Guid,
    /// The original Ship or Item category.
    pub entity_kind: LoadoutOwnerKind,
    /// The emitted Catalog row, including empty Ships when resolution succeeds.
    pub row: Option<Loadout>,
    /// The original physical sources and contributing uses for this owner.
    pub source_inventory: SourceInventory,
    /// Actual resolver and alignment operations in this owner's dependency scope.
    pub dependencies: LoadoutDependencyCoverage,
}

/// Terminal availability and coverage of the selected original Loadout owners.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct LoadoutVisitSummary {
    /// The original Archive Build, if declared.
    pub build: Option<Build>,
    /// Availability of the original Loadout sources.
    pub availability: DatasetAvailability,
    /// The number of admitted physical owners visited, including failed resolutions.
    pub owner_count: usize,
    /// The number of canonical Loadout rows delivered.
    pub row_count: usize,
    /// Original owner admission and read coverage.
    pub coverage: CatalogRecordCoverage,
    /// Explicit batch indices absent from this DataForge, in ascending order.
    pub missing_record_indices: Vec<u32>,
    /// Whether all consulted resolver and alignment scopes completed.
    pub dependencies_complete: bool,
}

/// Visits selected original Loadout owners in canonical GUID order with stable ties.
///
/// An admitted owner without a row carries its identity and dependency coverage. Commit staged
/// consumer changes after terminal success and coverage checks.
///
/// # Errors
///
/// Returns input, Archive-change, cancellation or visitor errors.
pub fn visit_loadouts<E: From<CatalogError>>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(&LoadoutOwnerSnapshot) -> Result<(), E>,
) -> Result<LoadoutVisitSummary, E> {
    visit_loadout_owners(
        archive,
        LoadoutOwnerSelection::Records(selection),
        cancel,
        visit,
    )
}

/// Visits resolved Loadout owners selected by root kind or physical Record identity.
/// Commit staged consumer changes after terminal success and coverage checks.
///
/// # Errors
/// Returns input, Archive-change, cancellation or visitor errors.
pub fn visit_loadout_owners<E: From<CatalogError>>(
    archive: &Archive,
    selection: LoadoutOwnerSelection<'_>,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(&LoadoutOwnerSnapshot) -> Result<(), E>,
) -> Result<LoadoutVisitSummary, E> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .visit_loadout_owners(selection, cancel, visit)
}

/// Visits selected Loadout owners using a decoded Entry from the same Archive.
///
/// # Errors
///
/// Returns query or visitor errors, including an origin mismatch for a foreign Entry.
pub fn visit_loadouts_from<E: From<CatalogError>>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(&LoadoutOwnerSnapshot) -> Result<(), E>,
) -> Result<LoadoutVisitSummary, E> {
    visit_loadout_owners_from(
        archive,
        entry,
        LoadoutOwnerSelection::Records(selection),
        cancel,
        visit,
    )
}

/// Visits resolved Loadout owners using a decoded Entry of the same Archive.
///
/// # Errors
/// Returns the errors of `visit_loadout_owners` or an origin mismatch.
pub fn visit_loadout_owners_from<E: From<CatalogError>>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: LoadoutOwnerSelection<'_>,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(&LoadoutOwnerSnapshot) -> Result<(), E>,
) -> Result<LoadoutVisitSummary, E> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .visit_loadout_owners(selection, cancel, visit)
}

impl CatalogQuerySession<'_> {
    /// Visits original Loadout owners from this reusable Archive session.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`visit_loadouts`].
    pub fn visit_loadouts<E: From<CatalogError>>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        visit: impl FnMut(&LoadoutOwnerSnapshot) -> Result<(), E>,
    ) -> Result<LoadoutVisitSummary, E> {
        self.visit_loadout_owners(LoadoutOwnerSelection::Records(selection), cancel, visit)
    }

    /// Visits selected resolved Loadout owners from this reusable Archive session.
    ///
    /// # Errors
    /// Returns the errors of `visit_loadout_owners`.
    pub fn visit_loadout_owners<E: From<CatalogError>>(
        &self,
        selection: LoadoutOwnerSelection<'_>,
        cancel: Option<&CancelToken>,
        visit: impl FnMut(&LoadoutOwnerSnapshot) -> Result<(), E>,
    ) -> Result<LoadoutVisitSummary, E> {
        self.parsed(cancel)?
            .visit_loadout_owners(selection, cancel, visit)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Visits original Loadout owners using this parsed DataForge.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`visit_loadouts`].
    pub fn visit_loadouts<E: From<CatalogError>>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        visit: impl FnMut(&LoadoutOwnerSnapshot) -> Result<(), E>,
    ) -> Result<LoadoutVisitSummary, E> {
        self.visit_loadout_owners(LoadoutOwnerSelection::Records(selection), cancel, visit)
    }

    /// Visits selected resolved Loadout owners using this parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of `visit_loadout_owners`.
    pub fn visit_loadout_owners<E: From<CatalogError>>(
        &self,
        selection: LoadoutOwnerSelection<'_>,
        cancel: Option<&CancelToken>,
        visit: impl FnMut(&LoadoutOwnerSnapshot) -> Result<(), E>,
    ) -> Result<LoadoutVisitSummary, E> {
        self.project_fallible(cancel, |archive, entry, forge| {
            visit_owners(
                &Query {
                    archive,
                    entry,
                    forge,
                    cancel,
                },
                selection,
                visit,
            )
        })
    }
}

fn visit_owners<E: From<CatalogError>>(
    query: &Query<'_>,
    selection: LoadoutOwnerSelection<'_>,
    mut visit: impl FnMut(&LoadoutOwnerSnapshot) -> Result<(), E>,
) -> Result<LoadoutVisitSummary, E> {
    let scope::VisitScope {
        owners: scope::Scope { wanted, coverage },
        missing_record_indices,
    } = scope::read_visit(query.forge, selection, query.cancel)?;
    let mut summary = LoadoutVisitSummary {
        build: query.archive.build().cloned(),
        availability: query.availability(0),
        owner_count: 0,
        row_count: 0,
        coverage,
        missing_record_indices,
        dependencies_complete: crate::export_archive_health::inventory(query.archive).is_none(),
    };
    if !wanted.is_empty() {
        let locale = TextSources::read_languages(query.archive, &[])?;
        let texts = Texts::parse(&locale)?;
        let inputs = ResolverInputs::new(query.archive, query.forge);
        for (_, want) in resolution::in_catalog_order(wanted.into_iter()) {
            checkpoint(query.cancel)?;
            let snapshot = read_owner(query, &inputs, &texts, want, &mut summary.coverage)?;
            checkpoint(query.cancel)?;
            visit(&snapshot)?;
            summary.owner_count += 1;
            summary.row_count += usize::from(snapshot.row.is_some());
            summary.dependencies_complete &= snapshot.dependencies.is_complete();
        }
        summary.availability = query.availability(summary.row_count);
    }
    scope::finish_coverage(&mut summary.coverage);
    Ok(summary)
}

fn read_owner(
    query: &Query<'_>,
    inputs: &ResolverInputs<'_>,
    texts: &Texts<'_>,
    want: Wanted,
    coverage: &mut CatalogRecordCoverage,
) -> Result<LoadoutOwnerSnapshot, CatalogError> {
    let origins = query.entry.records(query.archive)?;
    let input = OriginalCatalogInput::new(query.forge, &origins);
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(texts, &input, &registry).for_kind(Kind::Loadouts);
    let mut alignment = DirectAlignment::new(&projection, query.cancel);
    let mut row = None;
    let dependencies = query.resolve_prepared_rows(
        &input,
        inputs,
        std::iter::once(want),
        coverage,
        &mut alignment,
        |loadout| {
            if row.replace(loadout).is_some() {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            Ok(())
        },
    )?;
    Ok(LoadoutOwnerSnapshot {
        record_index: want.record_index,
        entity_guid: want.guid.0,
        entity_kind: want.kind,
        row,
        source_inventory: origins.uses.finish_with(|| checkpoint(query.cancel))?,
        dependencies,
    })
}
