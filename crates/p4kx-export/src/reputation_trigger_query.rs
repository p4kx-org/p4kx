// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory};

use super::unlocalized_record_query::plan::Plan;
use super::{CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_query_session::{CatalogQuerySession, ParsedCatalogQueries, checkpoint};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::reputation_faction_definition;
use crate::guid_text::CatalogGuid;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, Kind};

pub use crate::catalog_v3::reputation_trigger_configuration::ReputationTriggerConfiguration;

/// Selected original faction-owned trigger configurations and their source evidence.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct ReputationTriggerSnapshot<'source> {
    /// The identified Archive Build, when available.
    pub build: Option<Build>,
    /// Availability of the original faction reputation Record family.
    pub availability: DatasetAvailability,
    /// Canonical configurations ordered by owner GUID and physical Record index.
    pub rows: Vec<ReputationTriggerConfiguration<'source>>,
    /// Unreadable and skipped original Records within the requested scope.
    pub coverage: CatalogRecordCoverage,
    /// Original sources and their uses referenced by the configurations.
    pub source_inventory: SourceInventory,
}

/// Supplies authored sandbox trigger slots on selected faction reputation Records.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_reputation_triggers<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ReputationTriggerSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_reputation_triggers(selection, cancel, inspect)
}

/// Supplies trigger configurations using an already decoded Entry of the same Archive.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_reputation_triggers_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ReputationTriggerSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_reputation_triggers(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies original faction-owned triggers through this Archive session.
    ///
    /// # Errors
    /// Returns the errors of [`with_reputation_triggers`].
    pub fn with_reputation_triggers<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ReputationTriggerSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_reputation_triggers(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies original faction-owned triggers from this parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_reputation_triggers`].
    pub fn with_reputation_triggers<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ReputationTriggerSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            with_parsed(archive, entry, forge, selection, cancel, inspect)
        })
    }
}

fn with_parsed<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ReputationTriggerSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let Plan {
        mut owners,
        mut coverage,
    } = Plan::read(forge, Kind::FactionReputations, selection, cancel, None)?;
    owners.sort_by_cached_key(|owner| {
        (
            CatalogGuid(owner.record.guid()).text(),
            owner.record.index(),
        )
    });
    let origins = CatalogOrigins::for_archive_records(archive, entry)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let mut rows = Vec::with_capacity(owners.len());
    for owner in owners {
        checkpoint(cancel)?;
        let admission = RecordObservation::capture(&input, &owner.record)?;
        let (row, notes) =
            ReputationTriggerConfiguration::project(&owner.tree, &input, &admission, cancel)?;
        if notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(owner.record.index());
        }
        rows.push(row);
    }
    let table = reputation_faction_definition::trigger_table(forge, rows);
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(ReputationTriggerSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        coverage,
        source_inventory,
    }))
}
