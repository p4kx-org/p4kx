// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory};

use super::unlocalized_record_query::plan::Plan;
use super::{CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_query_session::{CatalogQuerySession, ParsedCatalogQueries, checkpoint};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::reputation_reward_amount_definition;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, Kind};

pub use crate::catalog_v3::reputation_reward_amount_definition::ReputationRewardAmountDefinition;

/// Selected physical reputation reward amounts and their original evidence.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct ReputationRewardAmountSnapshot<'source> {
    /// The Build identified by the opened Archive, when available.
    pub build: Option<Build>,
    /// Availability of the original Record family.
    pub availability: DatasetAvailability,
    /// Canonical definitions in GUID and physical Record order.
    pub rows: Vec<ReputationRewardAmountDefinition<'source>>,
    /// The original sources and uses for the selected values.
    pub source_inventory: SourceInventory,
    /// Unreadable and skipped original Records within the selection.
    pub coverage: CatalogRecordCoverage,
}

/// Supplies selected original reputation reward amounts to a callback.
/// Publish consumer-owned results after this function succeeds.
///
/// # Errors
/// Returns Archive, accounting, source-evidence or cancellation errors.
pub fn with_reputation_reward_amounts<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ReputationRewardAmountSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_reputation_reward_amounts(selection, cancel, inspect)
}

/// Supplies original reward amounts using an already decoded Entry of this Archive.
///
/// # Errors
/// Returns the errors of [`with_reputation_reward_amounts`] or a foreign input error.
pub fn with_reputation_reward_amounts_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ReputationRewardAmountSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_reputation_reward_amounts(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies original reward amounts through this Archive session.
    ///
    /// # Errors
    /// Returns the errors of [`with_reputation_reward_amounts`].
    pub fn with_reputation_reward_amounts<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ReputationRewardAmountSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_reputation_reward_amounts(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies original reward amounts from the parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_reputation_reward_amounts`].
    pub fn with_reputation_reward_amounts<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ReputationRewardAmountSnapshot<'_>) -> T,
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
    inspect: impl FnOnce(ReputationRewardAmountSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let Plan {
        owners,
        mut coverage,
    } = Plan::read(
        forge,
        Kind::ReputationRewardAmounts,
        selection,
        cancel,
        None,
    )?;
    let origins = CatalogOrigins::for_archive_records(archive, entry)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let mut rows = Vec::with_capacity(owners.len());
    for owner in owners {
        checkpoint(cancel)?;
        let admission = RecordObservation::capture(&input, &owner.record)?;
        let read =
            reputation_reward_amount_definition::read(&owner.tree, &input, &admission, cancel)?;
        if read
            .notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(owner.record.index());
        }
        rows.push(read.value);
    }
    reputation_reward_amount_definition::order(&mut rows);
    let table = reputation_reward_amount_definition::table(forge, rows);
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(ReputationRewardAmountSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        source_inventory,
        coverage,
    }))
}
