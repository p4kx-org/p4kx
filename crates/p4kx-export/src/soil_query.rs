// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to original loose soil definitions.

use std::collections::HashSet;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, ArchiveWatch, CancelToken};

use super::loose_definitions::{self, LooseDefinitionSnapshot};
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::loose_archive_path;
pub use crate::catalog_read::loose_definition::{DefinitionIssue, LooseDefinitionSelection};
use crate::catalog_read::original::{EntryCapture, with_entry_capture};
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::catalog_definitions::{planet_tech_definitions, soil, soil_texture_set};
use crate::export_archive_health;
use crate::export_ledger::ExportLedger;
use crate::{CatalogError, CatalogOptions, Kind};

pub use crate::catalog_read::catalog_json_number::CatalogJsonNumber;
pub use crate::catalog_read::loose_archive_path::{LooseArchivePath, LooseArchivePathState};
pub use crate::catalog_read::original::{PhysicalEntryIdentity, PhysicalEntryRef};
pub use crate::guid_text::CatalogGuid;
pub use soil::{
    GroundLayer, LayerTextureLink, LayerTextureLinks, RawPlanetAssetMember, Soil,
    TerrainGuidCandidate, TerrainGuidLink, TerrainGuidLinkState, TextureLayer,
};
pub use soil_texture_set::SoilTextureSet;

/// Selected original soils and the evidence for their physical owner Entries.
pub type SoilSnapshot = LooseDefinitionSnapshot<Soil>;

/// Selected original soil texture sets and their physical Entry evidence.
pub type SoilTextureSetSnapshot = LooseDefinitionSnapshot<SoilTextureSet>;

/// Reads selected soils and their authored texture-set links.
///
/// Nonzero links consult the canonical terrain GUID families. Duplicate diagnostics
/// cover the returned owner rows.
///
/// # Errors
/// Returns Archive-change, source-accounting or cancellation errors.
pub fn read_soils(
    archive: &Archive,
    selection: LooseDefinitionSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<SoilSnapshot, CatalogError> {
    read(
        archive,
        selection,
        cancel,
        Kind::Soils,
        |capture, ledger, options| {
            let mut table = soil::read_selected(archive, capture, ledger, options, selection)?;
            let wanted = wanted_texture_sets(&table.rows, cancel)?;
            let index = planet_tech_definitions::TerrainGuidIndex::read(
                archive,
                capture,
                cancel,
                None,
                planet_tech_definitions::terrain_target_families(),
                &wanted,
            )?;
            for row in &mut table.rows {
                checkpoint(cancel)?;
                row.link(&index, capture, ledger)?;
            }
            Ok(table)
        },
    )
}

/// Reads selected original soil texture sets and their texture path directory links.
///
/// Duplicate diagnostics cover the returned owner rows.
///
/// # Errors
/// Returns Archive-change, source-accounting or cancellation errors.
pub fn read_soil_texture_sets(
    archive: &Archive,
    selection: LooseDefinitionSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<SoilTextureSetSnapshot, CatalogError> {
    read(
        archive,
        selection,
        cancel,
        Kind::SoilTextureSets,
        |capture, ledger, options| {
            let mut paths = loose_archive_path::Resolver::new(archive, capture, cancel);
            soil_texture_set::read_selected(
                archive, capture, ledger, options, &mut paths, selection,
            )
        },
    )
}

fn wanted_texture_sets(
    rows: &[Soil],
    cancel: Option<&CancelToken>,
) -> Result<HashSet<CatalogGuid>, CatalogError> {
    let mut wanted = HashSet::new();
    for guid in rows.iter().flat_map(Soil::required_guids) {
        checkpoint(cancel)?;
        wanted
            .try_reserve(1)
            .map_err(CatalogError::JsonSourceAllocation)?;
        wanted.insert(guid);
    }
    Ok(wanted)
}

fn read<Row>(
    archive: &Archive,
    selection: LooseDefinitionSelection<'_>,
    cancel: Option<&CancelToken>,
    kind: Kind,
    read_rows: impl FnOnce(
        &EntryCapture<'_>,
        &mut ExportLedger,
        &CatalogOptions<'_>,
    ) -> Result<CatalogTable<Row>, CatalogError>,
) -> Result<LooseDefinitionSnapshot<Row>, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let snapshot = with_entry_capture(archive, |capture| {
        let mut ledger = ExportLedger {
            archive_inventory_problem: export_archive_health::inventory(archive),
            ..ExportLedger::default()
        };
        let mut options = CatalogOptions::new(UNIX_EPOCH);
        options.cancel = cancel;
        let table = read_rows(capture, &mut ledger, &options)?;
        loose_definitions::finish(archive, capture, selection, cancel, kind, table, ledger)
    });
    watch.check()?;
    checkpoint(cancel)?;
    snapshot
}
