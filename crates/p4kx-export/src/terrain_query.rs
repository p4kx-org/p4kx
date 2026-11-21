// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to original loose terrain definitions.

use p4kx_extraction::{Archive, CancelToken};

use super::loose_definitions::{self, LooseDefinitionSnapshot};
use crate::catalog_read::loose_archive_path::Resolver;
pub use crate::catalog_read::loose_archive_path::{LooseArchivePath, LooseArchivePathState};
use crate::catalog_v3::catalog_definitions::{ecosystem, shading_class};
use crate::{CatalogError, Kind};

pub use crate::catalog_read::catalog_json_number::CatalogJsonNumber;
pub use crate::catalog_read::loose_definition::{DefinitionIssue, LooseDefinitionSelection};
pub use crate::catalog_read::original::{PhysicalEntryIdentity, PhysicalEntryRef};
pub use crate::catalog_v3::catalog_definitions::planet_asset_raw_fields::RawPlanetAssetMember;
pub use crate::catalog_v3::catalog_definitions::terrain_layers::GroundLayer;
pub use crate::guid_text::CatalogGuid;
pub use ecosystem::{Ecosystem, EcosystemSoundEffect, EcosystemTextureLinks, EcosystemTextures};
pub use shading_class::ShadingClass;

/// Selected original ecosystem definitions and their physical Entry evidence.
pub type EcosystemSnapshot = LooseDefinitionSnapshot<Ecosystem>;

/// Selected original terrain shading classes and their physical Entry evidence.
pub type ShadingClassSnapshot = LooseDefinitionSnapshot<ShadingClass>;

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
    loose_definitions::read(
        archive,
        selection,
        cancel,
        Kind::Ecosystems,
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
    loose_definitions::read(
        archive,
        selection,
        cancel,
        Kind::ShadingClasses,
        |capture, ledger, options| {
            let mut paths = Resolver::new(archive, capture, cancel);
            shading_class::read_selected(archive, capture, ledger, options, &mut paths, selection)
        },
    )
}
