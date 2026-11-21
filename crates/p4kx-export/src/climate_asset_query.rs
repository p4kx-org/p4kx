// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to original loose climate-asset definitions.

use std::cell::OnceCell;

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::lazy_loose_records::LazyLooseRecords;
use super::loose_definitions::{self, LooseDefinitionSnapshot};
use super::{CatalogLanguageSelection, CatalogQuerySession, ParsedCatalogQueries};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_query_session::checkpoint;
pub use crate::catalog_read::catalog_json_number::CatalogJsonNumber;
pub use crate::catalog_read::loose_archive_path::LooseArchivePathState;
pub use crate::catalog_read::loose_definition::{DefinitionIssue, LooseDefinitionSelection};
use crate::catalog_read::loose_record_link::{self, GuidRecordLookup};
pub use crate::catalog_read::loose_record_link::{LooseRecordCandidate, LooseRecordLink};
use crate::catalog_read::original::EntryCapture;
pub use crate::catalog_read::original::{PhysicalEntryIdentity, PhysicalEntryRef};
use crate::catalog_table::CatalogTable;
pub use crate::catalog_v3::catalog_definitions::planet_asset_raw_fields::RawPlanetAssetMember;
use crate::catalog_v3::catalog_definitions::planet_climate_asset;
pub use crate::catalog_v3::catalog_definitions::planet_climate_asset::PlanetClimateAsset;
pub use crate::catalog_v3::catalog_definitions::planet_climate_properties::{
    ClimateAppearance, ClimateEnvelope, ClimateGrowth, ClimateGrowthBucket, ClimateShadingLinks,
};
pub use crate::catalog_v3::catalog_definitions::planet_climate_subasset::ClimateSubAsset;
use crate::catalog_v3::catalog_definitions::planet_tech_definitions::{
    TerrainGuidIndex, requested_terrain_guids, terrain_target_families,
};
pub use crate::catalog_v3::catalog_definitions::terrain_guid_link::{
    TerrainGuidCandidate, TerrainGuidLink, TerrainGuidLinkState,
};
use crate::export_ledger::ExportLedger;
pub use crate::guid_text::CatalogGuid;
use crate::{CatalogError, CatalogOptions, Kind};

/// Selected original climate assets and the field evidence in their dependency scope.
pub type ClimateAssetSnapshot = LooseDefinitionSnapshot<PlanetClimateAsset>;

/// Reads selected climate assets and the links declared by those owners.
/// DataForge is opened only for an admitted sub-asset with a nonzero Record GUID.
///
/// # Errors
/// Returns DataForge, Archive-change, source-accounting or cancellation errors.
pub fn read_climate_assets(
    archive: &Archive,
    selection: LooseDefinitionSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<ClimateAssetSnapshot, CatalogError> {
    loose_definitions::read(
        archive,
        selection,
        cancel,
        Kind::ClimateAssets,
        |capture, ledger, options| {
            let entry = OnceCell::new();
            let records = LazyLooseRecords::new(archive, capture.source_uses(), cancel, &entry);
            read(
                archive,
                capture,
                ledger,
                options,
                selection,
                &mut |guid, selector, expected| {
                    records.with_input(|original| {
                        loose_record_link::resolve_guid(
                            guid, selector, original, expected, None, cancel,
                        )
                    })
                },
            )
        },
    )
}

/// Reads selected climate assets with an already decoded DataForge Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn read_climate_assets_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: LooseDefinitionSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<ClimateAssetSnapshot, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .read_climate_assets(selection, cancel)
}

impl CatalogQuerySession<'_> {
    /// Reads selected climate assets through this Archive session.
    ///
    /// # Errors
    /// Returns DataForge, Archive-change, source-accounting or cancellation errors.
    pub fn read_climate_assets(
        &self,
        selection: LooseDefinitionSelection<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ClimateAssetSnapshot, CatalogError> {
        self.parsed(cancel)?.read_climate_assets(selection, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads selected climate assets using this parsed DataForge for Record targets.
    ///
    /// # Errors
    /// Returns Archive-change, source-accounting or cancellation errors.
    pub fn read_climate_assets(
        &self,
        selection: LooseDefinitionSelection<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ClimateAssetSnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            loose_definitions::read(
                archive,
                selection,
                cancel,
                Kind::ClimateAssets,
                |capture, ledger, options| {
                    let origins = CatalogOrigins::for_archive_records_with_uses(
                        archive,
                        entry,
                        capture.source_uses().clone(),
                    )?;
                    let original = OriginalCatalogInput::new(forge, &origins);
                    read(
                        archive,
                        capture,
                        ledger,
                        options,
                        selection,
                        &mut |guid, selector, expected| {
                            loose_record_link::resolve_guid(
                                guid, selector, &original, expected, None, cancel,
                            )
                        },
                    )
                },
            )
        })
    }
}

fn read(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
    selection: LooseDefinitionSelection<'_>,
    lookup: &mut GuidRecordLookup<'_>,
) -> Result<CatalogTable<PlanetClimateAsset>, CatalogError> {
    let mut table =
        planet_climate_asset::read_selected(archive, capture, ledger, options, selection, lookup)?;
    let wanted = requested_terrain_guids(
        table
            .rows
            .iter()
            .flat_map(PlanetClimateAsset::required_shading_guids),
        options.cancel,
    )?;
    let index = TerrainGuidIndex::read(
        archive,
        capture,
        options.cancel,
        options.filter,
        terrain_target_families(),
        &wanted,
    )?;
    for row in &mut table.rows {
        checkpoint(options.cancel)?;
        row.link_shading(&index, capture, ledger)?;
    }
    Ok(table)
}
