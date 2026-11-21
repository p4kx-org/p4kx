// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to original particle libraries.

use p4kx_extraction::{Archive, CancelToken};

use super::loose_definitions::{self, LooseDefinitionSnapshot};
use crate::build_derived::particle_library;
pub use crate::build_derived::particle_library::{ParticleLibrary, ParticleLibraryElement};
use crate::catalog_read::loose_definition::Selection;
pub use crate::catalog_read::original::{PhysicalEntryIdentity, SourcedXmlAttribute};
use crate::{CatalogError, Kind};

/// Selects original particle libraries in the opened root Archive.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum ParticleLibrarySelection {
    /// Every admitted particle-library Entry in directory order.
    All,
    /// The Entry at this original root directory index.
    EntryIndex(u32),
}

/// Selected original particle libraries with scoped read and source evidence.
pub type ParticleLibrarySnapshot = LooseDefinitionSnapshot<ParticleLibrary>;

/// Reads selected particle libraries and their typed original XML fields.
///
/// Entry indices are local to the opened Archive. Read and parse failures are
/// counted in the snapshot. The query opens no DataForge or referenced media.
///
/// # Errors
/// Returns an Archive-change, source-accounting or cancellation error.
pub fn read_particle_libraries(
    archive: &Archive,
    selection: ParticleLibrarySelection,
    cancel: Option<&CancelToken>,
) -> Result<ParticleLibrarySnapshot, CatalogError> {
    let selection = match selection {
        ParticleLibrarySelection::All => Selection::All,
        ParticleLibrarySelection::EntryIndex(index) => Selection::EntryIndex(index),
    };
    loose_definitions::read(
        archive,
        selection,
        cancel,
        Kind::ParticleLibraries,
        |capture, ledger, options| {
            particle_library::read_selected(
                archive,
                selection.entries(archive),
                capture,
                ledger,
                options.filter,
                options.cancel,
            )
        },
    )
}
