// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to original dismantle process definitions.

use p4kx_extraction::{Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::unlocalized_record_query::plan::Plan;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection,
};
use crate::catalog_query_session::{
    CatalogQuerySession, CatalogQuerySource, ParsedCatalogQueries, checkpoint,
};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::industry_modules::dismantle_process_definition;
use crate::record_observation::RecordObservation;
use crate::record_read::{self, RecordAdmission};
use crate::{CatalogError, Kind};

pub use dismantle_process_definition::{
    DismantleBlueprint, DismantleConfiguration, DismantleProcessDefinition, DismantleTime,
};

/// Selected original dismantle process definitions and their physical source evidence.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct DismantleProcessSnapshot<'source> {
    /// The installed Archive Build, when known.
    pub build: Option<Build>,
    /// Availability of the original Record family.
    pub availability: DatasetAvailability,
    /// Canonical rows ordered by GUID then physical Record index.
    pub rows: Vec<DismantleProcessDefinition<'source>>,
    /// Original sources and uses backing the returned values.
    pub source_inventory: SourceInventory,
    /// Unreadable, incomplete and skipped Records within the requested scope.
    pub coverage: CatalogRecordCoverage,
}

/// Supplies original dismantle process definitions by their physical Record identity.
/// Publish consumer-owned results after this function succeeds.
///
/// # Errors
/// Returns Archive, accounting, source-evidence or cancellation errors.
pub fn with_dismantle_processes<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(DismantleProcessSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_dismantle_processes(selection, cancel, inspect)
}

/// Supplies original dismantle process definitions from an already decoded Archive Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_dismantle_processes_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(DismantleProcessSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_dismantle_processes(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected dismantle processes through this Archive session.
    ///
    /// # Errors
    /// Returns the errors of [`with_dismantle_processes`].
    pub fn with_dismantle_processes<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(DismantleProcessSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_dismantle_processes(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected dismantle processes from the parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_dismantle_processes`].
    pub fn with_dismantle_processes<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(DismantleProcessSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            project(archive, entry, forge, selection, cancel, inspect)
        })
    }
}

fn project<T>(
    archive: &Archive,
    entry: CatalogQuerySource<'_>,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(DismantleProcessSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let Plan {
        owners,
        mut coverage,
    } = Plan::read_with(forge, selection, cancel, None, |record, _, coverage| {
        Ok(
            match record_read::classify(record, Kind::DismantleProcesses) {
                Ok(RecordAdmission::Selected) => true,
                Ok(RecordAdmission::Leftover) => {
                    coverage.skipped_leftovers += 1;
                    false
                }
                Ok(RecordAdmission::Other) => false,
                Err(_) => {
                    coverage.unreadable_headers.push(record.index());
                    false
                }
            },
        )
    })?;
    let origins = entry.records(archive)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let mut rows = Vec::with_capacity(owners.len());
    for owner in owners {
        checkpoint(cancel)?;
        let admission = RecordObservation::capture(&input, &owner.record)?;
        let Some(projected) =
            dismantle_process_definition::read_selected(&owner.tree, &input, &admission, cancel)?
        else {
            continue;
        };
        if projected
            .notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(owner.record.index());
        }
        rows.push(projected.value);
    }
    let table = dismantle_process_definition::table(forge, rows);
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(DismantleProcessSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        source_inventory,
        coverage,
    }))
}
