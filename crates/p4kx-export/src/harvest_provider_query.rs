// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory};

use super::unlocalized_record_query;
use super::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries,
};
use crate::catalog_query_session::CatalogQuerySource;
use crate::catalog_v3::catalog_harvest_modules::harvest_provider::{
    self, HarvestableProviderPreset,
};
use crate::catalog_v3::catalog_harvest_modules::harvest_provider_dependencies::ProviderDependencies;
use crate::{CatalogError, Kind};

/// Selected provider presets and the original source uses of their mining dependencies.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct HarvestableProviderPresetSnapshot {
    /// The Build identified when the Archive was opened, if available.
    pub build: Option<Build>,
    /// Availability of the provider Record family.
    pub availability: DatasetAvailability,
    /// The selected rows in canonical GUID order, with physical order for ties.
    pub rows: Vec<HarvestableProviderPreset>,
    /// The sources and uses for the returned values.
    pub source_inventory: SourceInventory,
    /// Coverage of the admitted provider Records.
    pub coverage: CatalogRecordCoverage,
    /// Distinct consulted dependency failures.
    pub unreadable_dependencies: usize,
}

/// Reads canonical harvestable provider presets and the dependencies of the selected rows.
///
/// # Errors
///
/// Returns input, source evidence or cancellation errors. Recoverable gaps appear in coverage.
pub fn read_harvestable_provider_presets(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<HarvestableProviderPresetSnapshot, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .read_harvestable_provider_presets(selection, cancel)
}

/// Reads harvestable provider presets using an already decoded entry of the same Archive.
///
/// # Errors
///
/// Returns the errors of [`read_harvestable_provider_presets`] or
/// [`CatalogError::SourceOriginMismatch`] for an entry from another input.
pub fn read_harvestable_provider_presets_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<HarvestableProviderPresetSnapshot, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .read_harvestable_provider_presets(selection, cancel)
}

impl CatalogQuerySession<'_> {
    /// Reads the selected provider presets and their referenced mining definitions.
    ///
    /// # Errors
    ///
    /// Returns input, source evidence or cancellation errors. Recoverable gaps appear in coverage.
    pub fn read_harvestable_provider_presets(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<HarvestableProviderPresetSnapshot, CatalogError> {
        self.parsed(cancel)?
            .read_harvestable_provider_presets(selection, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads the selected provider presets and their referenced mining definitions.
    ///
    /// # Errors
    ///
    /// Returns input, source evidence or cancellation errors. Recoverable gaps appear in coverage.
    pub fn read_harvestable_provider_presets(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<HarvestableProviderPresetSnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read(archive, entry, forge, selection, cancel)
        })
    }
}

fn read(
    archive: &Archive,
    entry: CatalogQuerySource<'_>,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<HarvestableProviderPresetSnapshot, CatalogError> {
    let dependencies = ProviderDependencies::default();
    let projected = unlocalized_record_query::read(
        archive,
        entry,
        forge,
        Kind::HarvestableProviderPresets,
        selection,
        cancel,
        |tree, input, admission| {
            harvest_provider::read(tree, input.forge(), input, admission, cancel, &dependencies)
        },
    )?;
    Ok(HarvestableProviderPresetSnapshot {
        build: archive.build().cloned(),
        availability: projected.availability,
        rows: projected.rows,
        source_inventory: projected.source_inventory,
        coverage: projected.coverage,
        unreadable_dependencies: dependencies.unreadable_count()?,
    })
}
