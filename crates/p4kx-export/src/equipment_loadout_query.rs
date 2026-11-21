// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Build, CancelToken, DataForgeEntry, SourceInventory};

use super::dependency_coverage::EquipmentDependencyCoverage;
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries,
};

#[path = "equipment_crafter_host_query.rs"]
mod crafter_host;
#[path = "equipment_loadout_read.rs"]
pub(super) mod read;
pub use crafter_host::{
    CrafterHost, CrafterHostSnapshot, read_crafter_hosts, read_crafter_hosts_from,
};

#[path = "equipment_loadout_scope.rs"]
mod scope;

pub use crate::contract_loadout::{EntityKind as LoadoutOwnerKind, Loadout, LoadoutPort, PortType};
pub use crate::loadout::alignment::direct::{
    AlignmentDependency, AlignmentDependencyState, AlignmentRole,
};

/// Resolved canonical loadouts and their result-local source inventory.
#[derive(Debug)]
#[non_exhaustive]
pub struct LoadoutSnapshot {
    /// The Build of the original Archive.
    pub build: Option<Build>,
    /// Availability of the declared original family.
    pub availability: DatasetAvailability,
    /// Selected loadouts in Catalog order, retaining separate physical owners.
    pub rows: Vec<Loadout>,
    /// Immediate source uses and their physical sources.
    pub source_inventory: SourceInventory,
    /// Physical output owner selection and read coverage.
    pub coverage: CatalogRecordCoverage,
    /// Actual resolver and alignment dependency coverage.
    pub dependencies: LoadoutDependencyCoverage,
}

/// The required source operations and alignment roles reached by resolution.
#[derive(Debug)]
#[non_exhaustive]
pub struct LoadoutDependencyCoverage {
    /// Record and Archive Entry operations from the canonical resolver.
    pub resolver: EquipmentDependencyCoverage,
    /// Actual alignment reads in physical Record and role order.
    pub alignment: Vec<AlignmentDependency>,
}

impl LoadoutDependencyCoverage {
    /// Reports whether all required resolver and alignment dependency reads completed.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.resolver.is_complete()
            && self.alignment.iter().all(|dependency| {
                matches!(
                    dependency.state,
                    AlignmentDependencyState::Complete | AlignmentDependencyState::NotAdmitted
                )
            })
    }
}

/// Resolves loadouts for selected original Ship and Item Records.
///
/// # Errors
/// Returns Archive, source-evidence or cancellation errors.
pub fn read_loadouts(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<LoadoutSnapshot, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .read_loadouts(selection, cancel)
}

/// Resolves loadouts using a decoded DataForge Entry from the supplied Archive.
///
/// # Errors
/// Returns the query errors or an origin mismatch for a foreign Entry.
pub fn read_loadouts_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<LoadoutSnapshot, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .read_loadouts(selection, cancel)
}

impl CatalogQuerySession<'_> {
    /// Resolves selected loadouts using the session's original DataForge Entry.
    ///
    /// # Errors
    /// Returns Archive, source-evidence or cancellation errors.
    pub fn read_loadouts(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<LoadoutSnapshot, CatalogError> {
        self.parsed(cancel)?.read_loadouts(selection, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Resolves selected loadouts using the parsed original DataForge.
    ///
    /// # Errors
    /// Returns Archive, source-evidence or cancellation errors.
    pub fn read_loadouts(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<LoadoutSnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::Query {
                archive,
                entry,
                forge,
                selection,
                cancel,
            }
            .run()
        })
    }
}
