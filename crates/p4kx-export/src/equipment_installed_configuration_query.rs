// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForgeEntry, SourceInventory, SourceValueState,
};

use super::read::Query;
use super::{LoadoutDependencyCoverage, scope};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, DatasetAvailability,
    ParsedCatalogQueries,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::facility_loadout_placements::{
    ScopedFlightLinks, project_installed_configurations, read_owner_links, scoped_table,
};
use crate::contract_loadout::Loadout;
use crate::{CatalogError, ExportAccountingError};

pub use crate::catalog_v3::facility_loadout_placements::{
    FlightConfigurationKind, FlightConfigurationLink, InstalledConfiguration,
    InstalledConfigurationDependency,
};

/// Installed Flight Controller, IFCS and Thruster links reached from one physical Ship Record.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct InstalledConfigurationSnapshot {
    /// The Build of the original Archive.
    pub build: Option<Build>,
    /// Availability of the original flight component families.
    pub availability: DatasetAvailability,
    /// Whether the selected Loadout is present, absent or unreadable.
    pub loadout_state: SourceValueState,
    /// Installed rows in original Loadout order.
    pub rows: Vec<InstalledConfiguration>,
    /// Original sources and uses for the selected query.
    pub source_inventory: SourceInventory,
    /// Physical Ship and Loadout read coverage.
    pub loadout_coverage: CatalogRecordCoverage,
    /// Resolver and alignment dependencies of the selected Loadout.
    pub loadout_dependencies: LoadoutDependencyCoverage,
    /// Flight configuration read coverage, ordered by the physical owner's Record index.
    pub configuration_dependencies: Vec<InstalledConfigurationDependency>,
}

/// Reads one physical Ship's installed flight configurations.
///
/// # Errors
/// Returns input, source-evidence, Archive change or cancellation errors.
pub fn read_installed_configurations(
    archive: &Archive,
    ship_record_index: u32,
    cancel: Option<&CancelToken>,
) -> Result<InstalledConfigurationSnapshot, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .read_installed_configurations(ship_record_index, cancel)
}

/// Reads installed flight configurations using an already decoded Entry of the same Archive.
///
/// # Errors
/// Returns query errors or an origin mismatch for a foreign Entry.
pub fn read_installed_configurations_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    ship_record_index: u32,
    cancel: Option<&CancelToken>,
) -> Result<InstalledConfigurationSnapshot, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .read_installed_configurations(ship_record_index, cancel)
}

impl CatalogQuerySession<'_> {
    /// Reads a physical Ship's installed flight configurations through this session.
    ///
    /// # Errors
    /// Returns input, source-evidence, Archive change or cancellation errors.
    pub fn read_installed_configurations(
        &self,
        ship_record_index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<InstalledConfigurationSnapshot, CatalogError> {
        self.parsed(cancel)?
            .read_installed_configurations(ship_record_index, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads installed flight configurations through the parsed original DataForge.
    ///
    /// # Errors
    /// Returns query or source-evidence errors, Archive changes, or cancellation.
    pub fn read_installed_configurations(
        &self,
        ship_record_index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<InstalledConfigurationSnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            selected(
                &Query {
                    archive,
                    entry,
                    forge,
                    cancel,
                },
                ship_record_index,
            )
        })
    }
}

fn selected(
    query: &Query<'_>,
    ship_record_index: u32,
) -> Result<InstalledConfigurationSnapshot, CatalogError> {
    let scope::Scope {
        wanted,
        coverage: mut loadout_coverage,
    } = scope::read_ship(query.forge, ship_record_index, query.cancel)?;
    let admitted = !wanted.is_empty();
    let origins = query.entry.records(query.archive)?;
    let (loadouts, loadout_dependencies) =
        query.resolve(&origins, wanted, &mut loadout_coverage)?;
    let mut configurations = BTreeMap::new();
    let (loadout_state, projected) = match loadouts.as_slice() {
        [loadout] => (
            SourceValueState::Present,
            read_placements(query, &origins, loadout, &mut configurations)?,
        ),
        [] if admitted || !loadout_coverage.unreadable_headers.is_empty() => {
            (SourceValueState::Unreadable, Vec::new())
        }
        [] => (SourceValueState::Absent, Vec::new()),
        _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
    };
    let CatalogTable { availability, rows } = scoped_table(query.forge, projected);
    loadout_coverage.incomplete_fields.sort_unstable();
    loadout_coverage.incomplete_fields.dedup();
    let source_inventory = origins.uses.finish_with(|| checkpoint(query.cancel))?;
    checkpoint(query.cancel)?;
    Ok(InstalledConfigurationSnapshot {
        build: query.archive.build().cloned(),
        availability,
        loadout_state,
        rows,
        source_inventory,
        loadout_coverage,
        loadout_dependencies,
        configuration_dependencies: configurations
            .into_values()
            .map(|record| record.coverage)
            .collect(),
    })
}

fn read_placements(
    query: &Query<'_>,
    origins: &CatalogOrigins,
    loadout: &Loadout,
    configurations: &mut BTreeMap<u32, ScopedFlightLinks>,
) -> Result<Vec<InstalledConfiguration>, CatalogError> {
    let input = OriginalCatalogInput::new(query.forge, origins);
    project_installed_configurations(loadout, origins, query.cancel, &mut |index| {
        let entry = match configurations.entry(index) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                entry.insert(read_owner_links(query.forge, &input, index, query.cancel)?)
            }
        };
        Ok(entry.links.clone())
    })
}
