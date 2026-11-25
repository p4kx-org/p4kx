// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, ProvenanceMode};

use super::guid_linked_definition_read::GuidLinkedDefinitionRead;
use super::{CatalogQuerySession, LooseDefinitionSnapshot, ParsedCatalogQueries};
use crate::catalog_read::loose_definition::LooseDefinitionSelection;
pub use crate::catalog_read::loose_record_link::{LooseRecordCandidate, LooseRecordLink};
use crate::catalog_v3::catalog_definitions::planet_geology_asset;
pub use crate::catalog_v3::catalog_definitions::planet_geology_asset::PlanetGeologyAsset;
pub use crate::catalog_v3::catalog_definitions::planet_geology_properties::{
    GeologyGeometry, GeologyPlacement, GeologySubstrate, SunShadowMode,
};
pub use crate::catalog_v3::catalog_definitions::planet_geology_references::GeologyReferences;
use crate::{CatalogError, Kind};

const QUERY: GuidLinkedDefinitionRead<PlanetGeologyAsset> =
    GuidLinkedDefinitionRead::new(Kind::GeologyAssets, planet_geology_asset::read_selected);

/// Selected original geology assets and their physical source evidence.
pub type GeologyAssetSnapshot = LooseDefinitionSnapshot<PlanetGeologyAsset>;

/// Reads selected original geology assets and their Entity class Record headers.
///
/// DataForge is opened only for a valid nonzero Entity class GUID in the selected definitions.
/// Geometry and material paths use only original Archive directory evidence.
///
/// # Errors
/// Returns DataForge, Archive-change, source-accounting or cancellation errors.
pub fn read_geology_assets(
    archive: &Archive,
    selection: LooseDefinitionSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<GeologyAssetSnapshot, CatalogError> {
    read_geology_assets_with_provenance_mode(
        archive,
        selection,
        ProvenanceMode::MarkersOnly,
        cancel,
    )
}

/// Reads selected planet geology assets with the requested source detail.
///
/// # Errors
/// Returns the errors of `read_geology_assets`.
pub fn read_geology_assets_with_provenance_mode(
    archive: &Archive,
    selection: LooseDefinitionSelection<'_>,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<GeologyAssetSnapshot, CatalogError> {
    QUERY.read(archive, selection, provenance_mode, cancel)
}

/// Reads selected geology assets with an already decoded DataForge Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn read_geology_assets_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: LooseDefinitionSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<GeologyAssetSnapshot, CatalogError> {
    QUERY.read_from(archive, entry, selection, cancel)
}

impl CatalogQuerySession<'_> {
    /// Reads selected original geology assets through this Archive session.
    ///
    /// # Errors
    /// Returns DataForge, Archive-change, source-accounting or cancellation errors.
    pub fn read_geology_assets(
        &self,
        selection: LooseDefinitionSelection<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<GeologyAssetSnapshot, CatalogError> {
        self.parsed(cancel)?.read_geology_assets(selection, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads selected original geology assets using this parsed DataForge.
    ///
    /// # Errors
    /// Returns Archive-change, source-accounting or cancellation errors.
    pub fn read_geology_assets(
        &self,
        selection: LooseDefinitionSelection<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<GeologyAssetSnapshot, CatalogError> {
        QUERY.read_parsed(self, selection, cancel)
    }
}
