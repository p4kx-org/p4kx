// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to original loose terrain definitions.

use p4kx_extraction::{Archive, CancelToken, ProvenanceMode};

pub use super::geology_asset_query::{
    GeologyAssetSnapshot, GeologyGeometry, GeologyPlacement, GeologyReferences, GeologySubstrate,
    LooseRecordCandidate, LooseRecordLink, PlanetGeologyAsset, SunShadowMode, read_geology_assets,
    read_geology_assets_from, read_geology_assets_with_provenance_mode,
};
use super::loose_definitions::{self, LooseDefinitionSnapshot};
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::loose_archive_path::Resolver;
pub use crate::catalog_read::loose_archive_path::{LooseArchivePath, LooseArchivePathState};
use crate::catalog_v3::catalog_definitions::{
    ecosystem, geology_set, geology_texture_set, planet_tech_definitions, shading_class,
};
use crate::{CatalogError, Kind};

pub use crate::catalog_read::catalog_json_number::CatalogJsonNumber;
pub use crate::catalog_read::loose_definition::{DefinitionIssue, LooseDefinitionSelection};
pub use crate::catalog_read::original::{PhysicalEntryIdentity, PhysicalEntryRef};
pub use crate::catalog_v3::catalog_definitions::planet_asset_raw_fields::RawPlanetAssetMember;
pub use crate::catalog_v3::catalog_definitions::terrain_guid_link::{
    TerrainGuidCandidate, TerrainGuidLink, TerrainGuidLinkState,
};
pub use crate::catalog_v3::catalog_definitions::terrain_layers::{
    GroundLayer, LayerTextureLink, LayerTextureLinks, TextureLayer,
};
pub use crate::guid_text::CatalogGuid;
pub use ecosystem::{Ecosystem, EcosystemSoundEffect, EcosystemTextureLinks, EcosystemTextures};
pub use geology_set::{GeologyAssetSlot, GeologySet, GeologySetLinks};
pub use geology_texture_set::GeologyTextureSet;
pub use shading_class::ShadingClass;

/// Selected original ecosystem definitions and their physical Entry evidence.
pub type EcosystemSnapshot = LooseDefinitionSnapshot<Ecosystem>;

/// Selected original terrain shading classes and their physical Entry evidence.
pub type ShadingClassSnapshot = LooseDefinitionSnapshot<ShadingClass>;

/// Selected original geology sets and their physical Entry evidence.
pub type GeologySetSnapshot = LooseDefinitionSnapshot<GeologySet>;

/// Selected original geology texture sets and their physical Entry evidence.
pub type GeologyTextureSetSnapshot = LooseDefinitionSnapshot<GeologyTextureSet>;

/// Reads selected original ecosystem definitions and their texture directory links.
/// Duplicate-owner issues describe only the physical owners admitted by this selection.
///
/// # Errors
/// Returns Archive-change, source-accounting or cancellation errors.
pub fn read_ecosystems(
    archive: &Archive,
    selection: LooseDefinitionSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<EcosystemSnapshot, CatalogError> {
    read_ecosystems_with_provenance_mode(archive, selection, ProvenanceMode::MarkersOnly, cancel)
}

/// Reads ecosystem definitions with the selected source detail.
///
/// # Errors
/// Returns the errors of `read_ecosystems`.
pub fn read_ecosystems_with_provenance_mode(
    archive: &Archive,
    selection: LooseDefinitionSelection<'_>,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<EcosystemSnapshot, CatalogError> {
    loose_definitions::read_with_provenance_mode(
        archive,
        selection,
        cancel,
        Kind::Ecosystems,
        provenance_mode,
        |capture, ledger, options| {
            let mut paths = Resolver::new(archive, capture, cancel);
            ecosystem::read_selected(archive, capture, ledger, options, &mut paths, selection)
        },
    )
}

/// Reads original terrain shading classes and their ground texture directory links.
/// Duplicate-owner issues describe only the physical owners admitted by this selection.
///
/// # Errors
/// Returns Archive-change, source-accounting or cancellation errors.
pub fn read_shading_classes(
    archive: &Archive,
    selection: LooseDefinitionSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<ShadingClassSnapshot, CatalogError> {
    read_shading_classes_with_provenance_mode(
        archive,
        selection,
        ProvenanceMode::MarkersOnly,
        cancel,
    )
}

/// Reads terrain shading classes with the selected source detail.
///
/// # Errors
/// Returns the errors of `read_shading_classes`.
pub fn read_shading_classes_with_provenance_mode(
    archive: &Archive,
    selection: LooseDefinitionSelection<'_>,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<ShadingClassSnapshot, CatalogError> {
    loose_definitions::read_with_provenance_mode(
        archive,
        selection,
        cancel,
        Kind::ShadingClasses,
        provenance_mode,
        |capture, ledger, options| {
            let mut paths = Resolver::new(archive, capture, cancel);
            shading_class::read_selected(archive, capture, ledger, options, &mut paths, selection)
        },
    )
}

/// Reads original geology sets and the targets of their authored terrain GUIDs.
/// Duplicate-owner issues describe only the physical owners admitted by this selection.
///
/// # Errors
/// Returns Archive-change, source-accounting or cancellation errors.
pub fn read_geology_sets(
    archive: &Archive,
    selection: LooseDefinitionSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<GeologySetSnapshot, CatalogError> {
    read_geology_sets_with_provenance_mode(archive, selection, ProvenanceMode::MarkersOnly, cancel)
}

/// Reads geology sets with the selected source detail.
///
/// # Errors
/// Returns the errors of `read_geology_sets`.
pub fn read_geology_sets_with_provenance_mode(
    archive: &Archive,
    selection: LooseDefinitionSelection<'_>,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<GeologySetSnapshot, CatalogError> {
    loose_definitions::read_with_provenance_mode(
        archive,
        selection,
        cancel,
        Kind::GeologySets,
        provenance_mode,
        |capture, ledger, options| {
            let mut table =
                geology_set::read_selected(archive, capture, ledger, options, selection)?;
            let wanted = planet_tech_definitions::requested_terrain_guids(
                table.rows.iter().flat_map(GeologySet::required_guids),
                cancel,
            )?;
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
                row.link(&index, capture, ledger, cancel)?;
            }
            Ok(table)
        },
    )
}

/// Reads original geology texture sets and their layer texture directory links.
/// Duplicate-owner issues describe only the physical owners admitted by this selection.
///
/// # Errors
/// Returns Archive-change, source-accounting or cancellation errors.
pub fn read_geology_texture_sets(
    archive: &Archive,
    selection: LooseDefinitionSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<GeologyTextureSetSnapshot, CatalogError> {
    read_geology_texture_sets_with_provenance_mode(
        archive,
        selection,
        ProvenanceMode::MarkersOnly,
        cancel,
    )
}

/// Reads geology texture sets with the selected source detail.
///
/// # Errors
/// Returns the errors of `read_geology_texture_sets`.
pub fn read_geology_texture_sets_with_provenance_mode(
    archive: &Archive,
    selection: LooseDefinitionSelection<'_>,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<GeologyTextureSetSnapshot, CatalogError> {
    loose_definitions::read_with_provenance_mode(
        archive,
        selection,
        cancel,
        Kind::GeologyTextureSets,
        provenance_mode,
        |capture, ledger, options| {
            let mut paths = Resolver::new(archive, capture, cancel);
            geology_texture_set::read_selected(
                archive, capture, ledger, options, &mut paths, selection,
            )
        },
    )
}
