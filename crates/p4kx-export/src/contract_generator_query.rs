// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Language, SourceInventory,
};

use super::{CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_query_session::{CatalogQuerySession, ParsedCatalogQueries, checkpoint};
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::mission_generator::GeneratorDefinition;

#[path = "contract_generator_query_notes.rs"]
mod notes;
#[path = "contract_generator_query_read.rs"]
mod read;

pub use notes::{ContractGeneratorProjectionFailure, ContractGeneratorProjectionNote};

/// Selected physical Generator definitions and their original source evidence.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct ContractGeneratorSnapshot<'source> {
    /// The installed Archive Build, when available.
    pub build: Option<Build>,
    /// Availability of the Generator source family.
    pub availability: DatasetAvailability,
    /// Complete source definitions in original physical Record order.
    pub rows: Vec<GeneratorDefinition<'source>>,
    /// Requested session language policy, independent of this unlocalized source query.
    pub requested_languages: Vec<Language>,
    /// Sources and uses captured for these definitions.
    pub source_inventory: SourceInventory,
    /// Coverage of the selected source owners and their typed field interpretations.
    pub coverage: CatalogRecordCoverage,
    /// Typed field notes retained from the canonical Generator projector.
    pub projection_notes: Vec<ContractGeneratorProjectionNote>,
    /// Failed mission-item interpretations whose complete original source remains present.
    pub projection_failures: Vec<ContractGeneratorProjectionFailure>,
}

/// Supplies selected original Generator definitions to a callback.
/// Publish consumer-owned changes after this function returns successfully.
///
/// # Errors
/// Returns input, source evidence or cancellation errors.
pub fn with_contract_generators<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractGeneratorSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .with_contract_generators(selection, cancel, inspect)
}

/// Supplies Generator definitions using an already decoded entry of the same Archive.
///
/// # Errors
/// Returns the errors of [`with_contract_generators`] or an origin mismatch for a foreign entry.
pub fn with_contract_generators_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractGeneratorSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_contract_generators(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies Generator definitions using this session's original DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_contract_generators`].
    pub fn with_contract_generators<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractGeneratorSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_contract_generators(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies Generator definitions using this view's parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_contract_generators`].
    pub fn with_contract_generators<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractGeneratorSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            with_parsed(
                archive,
                entry,
                forge,
                selection,
                self.languages(),
                cancel,
                inspect,
            )
        })
    }
}

fn with_parsed<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    requested_languages: &[Language],
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractGeneratorSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let origins = CatalogOrigins::for_archive_records(archive, entry)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let selected = read::read(forge, &input, selection, cancel)?;
    let table = CatalogTable::new(forge, &["ContractGenerator"], selected.rows);
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(ContractGeneratorSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        requested_languages: requested_languages.to_vec(),
        source_inventory,
        coverage: selected.coverage,
        projection_notes: selected.notes,
        projection_failures: selected.failures,
    }))
}
