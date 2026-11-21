// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to original loose creature definitions.

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::guid_linked_definition_read::GuidLinkedDefinitionRead;
use super::{CatalogQuerySession, LooseDefinitionSnapshot, ParsedCatalogQueries};
pub use crate::catalog_read::catalog_json_number::CatalogJsonNumber;
pub use crate::catalog_read::loose_archive_path::LooseArchivePathState;
pub use crate::catalog_read::loose_definition::{DefinitionIssue, LooseDefinitionSelection};
pub use crate::catalog_read::loose_record_link::{LooseRecordCandidate, LooseRecordLink};
pub use crate::catalog_read::original::PhysicalEntryIdentity;
use crate::catalog_v3::catalog_definitions::planet_creature;
pub use crate::catalog_v3::catalog_definitions::planet_creature::PlanetCreature;
pub use crate::guid_text::CatalogGuid;
use crate::{CatalogError, Kind};

const QUERY: GuidLinkedDefinitionRead<PlanetCreature> =
    GuidLinkedDefinitionRead::new(Kind::Creatures, planet_creature::read_selected);

/// Selected original creatures and their physical source evidence.
pub type CreatureSnapshot = LooseDefinitionSnapshot<PlanetCreature>;

/// Reads selected original creatures and their Entity class Record headers.
/// DataForge is opened only for a valid nonzero Entity class GUID in the selected definitions.
///
/// # Errors
/// Returns DataForge, Archive-change, source-accounting or cancellation errors.
pub fn read_creatures(
    archive: &Archive,
    selection: LooseDefinitionSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<CreatureSnapshot, CatalogError> {
    QUERY.read(archive, selection, cancel)
}

/// Reads selected creatures with an already decoded DataForge Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn read_creatures_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: LooseDefinitionSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<CreatureSnapshot, CatalogError> {
    QUERY.read_from(archive, entry, selection, cancel)
}

impl CatalogQuerySession<'_> {
    /// Reads selected original creatures through this Archive session.
    ///
    /// # Errors
    /// Returns DataForge, Archive-change, source-accounting or cancellation errors.
    pub fn read_creatures(
        &self,
        selection: LooseDefinitionSelection<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<CreatureSnapshot, CatalogError> {
        self.parsed(cancel)?.read_creatures(selection, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads selected original creatures using this parsed DataForge.
    ///
    /// # Errors
    /// Returns Archive-change, source-accounting or cancellation errors.
    pub fn read_creatures(
        &self,
        selection: LooseDefinitionSelection<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<CreatureSnapshot, CatalogError> {
        QUERY.read_parsed(self, selection, cancel)
    }
}
