// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to original medical item tier definitions.

use p4kx_extraction::{Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory};

use super::unlocalized_record_query::plan::Plan;
use super::{CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_query_session::{
    CatalogQuerySession, CatalogQuerySource, ParsedCatalogQueries, checkpoint,
};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::medical::medical_tier_config;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, Kind};

pub use crate::catalog_v3::catalog_capacity::ResourceCapacity;
pub use crate::catalog_v3::medical::med_bed_resource_consumption::MedBedResourceConsumption;
pub use crate::catalog_v3::medical::medical_tier::MedBedTier;
pub use crate::catalog_v3::medical::medical_tier_config::MedicalItemTierConfig;

/// Selected original medical tier definitions and their physical source evidence.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct MedicalItemTierSnapshot<'source> {
    /// The installed Archive Build, when known.
    pub build: Option<Build>,
    /// Availability of the original Record family.
    pub availability: DatasetAvailability,
    /// Canonical rows ordered by GUID then physical Record index.
    pub rows: Vec<MedicalItemTierConfig<'source>>,
    /// Original sources and uses backing the returned values.
    pub source_inventory: SourceInventory,
    /// Unreadable and skipped Records within the requested scope.
    pub coverage: CatalogRecordCoverage,
}

/// Supplies original medical tier definitions by their physical Record identity.
/// Publish consumer-owned results after this function succeeds.
///
/// # Errors
/// Returns Archive, accounting, source-evidence or cancellation errors.
pub fn with_medical_item_tier_configs<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MedicalItemTierSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_medical_item_tier_configs(selection, cancel, inspect)
}

/// Supplies original medical tier definitions from an already decoded Archive Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_medical_item_tier_configs_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MedicalItemTierSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_medical_item_tier_configs(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected medical definitions through this Archive session.
    ///
    /// # Errors
    /// Returns the errors of [`with_medical_item_tier_configs`].
    pub fn with_medical_item_tier_configs<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(MedicalItemTierSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_medical_item_tier_configs(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected medical definitions from the parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_medical_item_tier_configs`].
    pub fn with_medical_item_tier_configs<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(MedicalItemTierSnapshot<'_>) -> T,
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
    inspect: impl FnOnce(MedicalItemTierSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let Plan {
        owners,
        mut coverage,
    } = Plan::read(forge, Kind::MedicalItemTierConfigs, selection, cancel, None)?;
    let origins = entry.records(archive)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let mut rows = Vec::with_capacity(owners.len());
    for owner in owners {
        checkpoint(cancel)?;
        let admission = RecordObservation::capture(&input, &owner.record)?;
        let (row, notes) = medical_tier_config::read(&owner.tree, &input, &admission, cancel)?;
        if notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(owner.record.index());
        }
        rows.push(row);
    }
    medical_tier_config::order(&mut rows);
    let table = medical_tier_config::table(forge, rows);
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(MedicalItemTierSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        source_inventory,
        coverage,
    }))
}
