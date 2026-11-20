// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to original cargo manifest definitions.

use p4kx_extraction::{Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory};

use super::unlocalized_record_query::plan::Plan;
use super::{CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_query_session::{CatalogQuerySession, ParsedCatalogQueries, checkpoint};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::cargo_manifest;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, Kind};

pub use cargo_manifest::{
    CargoFillCapacity, CargoManifestDefinition, CargoQuantityAlgorithm, CargoResource,
    CargoResourceSlot, CargoSpawnRule,
};

/// Selected original cargo manifest definitions and their physical source evidence.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct CargoManifestSnapshot<'source> {
    /// The installed Archive Build, when known.
    pub build: Option<Build>,
    /// Availability of the original Record family.
    pub availability: DatasetAvailability,
    /// Canonical rows ordered by GUID then physical Record index.
    pub rows: Vec<CargoManifestDefinition<'source>>,
    /// Original sources and uses backing the returned values.
    pub source_inventory: SourceInventory,
    /// Unreadable, incomplete and skipped Records within the requested scope.
    pub coverage: CatalogRecordCoverage,
}

/// Supplies original cargo manifest definitions by their physical Record identity.
/// Publish consumer-owned results after this function succeeds.
///
/// # Errors
/// Returns Archive, accounting, source-evidence or cancellation errors.
pub fn with_cargo_manifests<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(CargoManifestSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_cargo_manifests(selection, cancel, inspect)
}

/// Supplies original cargo manifest definitions from an already decoded Archive Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_cargo_manifests_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(CargoManifestSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_cargo_manifests(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected cargo manifests through this Archive session.
    ///
    /// # Errors
    /// Returns the errors of [`with_cargo_manifests`].
    pub fn with_cargo_manifests<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(CargoManifestSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_cargo_manifests(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected cargo manifests from the parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_cargo_manifests`].
    pub fn with_cargo_manifests<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(CargoManifestSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            project(archive, entry, forge, selection, cancel, inspect)
        })
    }
}

fn project<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(CargoManifestSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let Plan {
        owners,
        mut coverage,
    } = Plan::read(forge, Kind::CargoManifests, selection, cancel, None)?;
    let origins = CatalogOrigins::for_archive_records(archive, entry)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let mut rows = Vec::with_capacity(owners.len());
    for owner in owners {
        checkpoint(cancel)?;
        let admission = RecordObservation::capture(&input, &owner.record)?;
        let (row, notes) = cargo_manifest::read(&owner.tree, &input, &admission, cancel)?;
        if notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(owner.record.index());
        }
        rows.push(row);
    }
    let table = cargo_manifest::table(forge, rows);
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(CargoManifestSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        source_inventory,
        coverage,
    }))
}
