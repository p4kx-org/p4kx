// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selected physical contract scenario progressions.

use p4kx_extraction::{Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory};

use super::unlocalized_record_query::plan::Plan;
use super::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_query_session::{
    CatalogQuerySession, CatalogQuerySource, ParsedCatalogQueries, checkpoint,
};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::mission_sources::scenario_progress;
use crate::export_diagnostic::ExportSourceRole;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, Kind};

pub use scenario_progress::{
    ContractScenario, ScenarioFactionTiers, ScenarioProgression, ScenarioTier,
};

/// Selected progression rows and their original source inventory.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct ContractScenarioSnapshot<'source> {
    /// Build of the read Archive, when supplied.
    pub build: Option<Build>,
    /// Availability of the original `ScenarioProgress` declaration.
    pub availability: DatasetAvailability,
    /// Canonical rows in GUID and physical Record order.
    pub rows: Vec<ContractScenario<'source>>,
    /// Selection and read coverage for the requested progression Records.
    pub coverage: CatalogRecordCoverage,
    /// Original source and use inventory of the selected projection.
    pub source_inventory: SourceInventory,
}

/// Supplies the canonical contract progression rows for the selected physical Records.
///
/// # Errors
/// Returns Archive, accounting or cancellation errors.
pub fn with_contract_scenarios<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractScenarioSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(
        archive,
        super::CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_contract_scenarios(selection, cancel, inspect)
}

/// Supplies canonical progression rows from an already decoded Archive Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_contract_scenarios_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractScenarioSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        super::CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_contract_scenarios(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected progression Records through this session.
    ///
    /// # Errors
    /// Returns the errors of [`with_contract_scenarios`].
    pub fn with_contract_scenarios<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractScenarioSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_contract_scenarios(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected progression Records from this parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_contract_scenarios`].
    pub fn with_contract_scenarios<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractScenarioSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            selected(archive, entry, forge, selection, cancel, inspect)
        })
    }
}

fn selected<T>(
    archive: &Archive,
    entry: CatalogQuerySource<'_>,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractScenarioSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let Plan {
        owners,
        mut coverage,
    } = Plan::read(forge, Kind::ContractScenarios, selection, cancel, None)?;
    let origins = entry.records(archive)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let mut rows = Vec::with_capacity(owners.len());
    for owner in owners {
        checkpoint(cancel)?;
        let admission = RecordObservation::capture(&input, &owner.record)?;
        let read = scenario_progress::read(&owner.tree, &input, &admission, cancel)?;
        let owner_ledger = crate::export_field_walk::inspect_observed(
            Some(&input),
            &owner.tree,
            &admission,
            Some(Kind::ContractScenarios),
            ExportSourceRole::Selected,
            cancel,
        )?;
        let incomplete = !owner_ledger.field_projection_complete(0, 0)
            || read
                .notes
                .iter()
                .any(|note| note.description().issue.blocks_projection())
            || !read.dependencies.field_projection_complete(0, 0)
            || read
                .dependencies
                .sources
                .iter()
                .any(|source| source.disposition.failed());
        if incomplete {
            coverage.incomplete_fields.push(owner.record.index());
        }
        rows.push(read.scenario);
    }
    let rows = scenario_progress::ordered_rows(rows);
    let availability = DatasetAvailability::from_source_declarations(forge, |name| {
        name == scenario_progress::SOURCE_TYPE
    });
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(ContractScenarioSnapshot {
        build: archive.build().cloned(),
        availability,
        rows,
        coverage,
        source_inventory,
    }))
}
