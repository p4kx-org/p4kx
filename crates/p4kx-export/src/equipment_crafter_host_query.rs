// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory, SourceValueState,
    TreeBuilder,
};

use super::read::Query;
use super::{LoadoutDependencyCoverage, scope};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_queries::crafting::read_station;
use crate::catalog_queries::unlocalized_record_query::plan::{crafter_admission, read_owner};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::industry_modules::crafter_host_projection;
use crate::contract_loadout::Loadout;
use crate::scan::CRAFTER_STRUCT;
use crate::{CatalogError, ExportAccountingError};

pub use crafter_host_projection::CrafterHost;

/// Resolved crafter mounts on one selected original ship Loadout.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct CrafterHostSnapshot {
    /// The original Archive Build, when known.
    pub build: Option<Build>,
    /// Availability of the original crafter component family.
    pub availability: DatasetAvailability,
    /// The selected ship's Loadout state, independently of the mount count.
    pub host_state: SourceValueState,
    /// The mounted crafters ordered by physical Record index.
    pub rows: Vec<CrafterHost>,
    /// Original sources and uses of the returned rows.
    pub source_inventory: SourceInventory,
    /// Coverage of the selected ship and resolved Loadout.
    pub loadout_coverage: CatalogRecordCoverage,
    /// Coverage of the reached crafter candidates.
    pub crafter_coverage: CatalogRecordCoverage,
    /// Resolver and alignment dependency coverage.
    pub dependencies: LoadoutDependencyCoverage,
}

/// Reads crafter mounts from one physical ship Record.
///
/// # Errors
/// Returns input, source evidence, Archive-change or cancellation errors.
pub fn read_crafter_hosts(
    archive: &Archive,
    ship_record_index: u32,
    cancel: Option<&CancelToken>,
) -> Result<CrafterHostSnapshot, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .read_crafter_hosts(ship_record_index, cancel)
}

/// Reads the selected ship's crafter mounts using an already decoded Archive Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn read_crafter_hosts_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    ship_record_index: u32,
    cancel: Option<&CancelToken>,
) -> Result<CrafterHostSnapshot, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .read_crafter_hosts(ship_record_index, cancel)
}

impl CatalogQuerySession<'_> {
    /// Reads one ship's crafter mounts through the current session.
    ///
    /// # Errors
    /// Returns input, source evidence, Archive-change or cancellation errors.
    pub fn read_crafter_hosts(
        &self,
        ship_record_index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<CrafterHostSnapshot, CatalogError> {
        self.parsed(cancel)?
            .read_crafter_hosts(ship_record_index, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads one ship's crafter mounts through the parsed original DataForge.
    ///
    /// # Errors
    /// Returns input, source evidence, Archive-change or cancellation errors.
    pub fn read_crafter_hosts(
        &self,
        ship_record_index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<CrafterHostSnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            selected(
                &Query {
                    archive,
                    entry,
                    forge,
                    selection: CatalogRecordSelection::RecordIndex(ship_record_index),
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
) -> Result<CrafterHostSnapshot, CatalogError> {
    let scope::Scope {
        wanted,
        coverage: mut loadout_coverage,
    } = scope::read_ship(query.forge, ship_record_index, query.cancel)?;
    let admitted = !wanted.is_empty();
    let origins = CatalogOrigins::for_archive_records(query.archive, query.entry)?;
    let (loadouts, dependencies) = query.resolve(&origins, wanted, &mut loadout_coverage)?;
    let mut crafter_coverage = CatalogRecordCoverage::default();
    let (host_state, rows) = match loadouts.as_slice() {
        [loadout] => {
            let input = OriginalCatalogInput::new(query.forge, &origins);
            let rows = mounts(
                loadout,
                query.forge,
                &input,
                query.cancel,
                &mut crafter_coverage,
            )?;
            (SourceValueState::Present, rows)
        }
        [] if admitted || !loadout_coverage.unreadable_headers.is_empty() => {
            (SourceValueState::Unreadable, Vec::new())
        }
        [] => (SourceValueState::Absent, Vec::new()),
        _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
    };
    loadout_coverage.incomplete_fields.sort_unstable();
    loadout_coverage.incomplete_fields.dedup();
    let availability =
        DatasetAvailability::from_source_declarations(query.forge, |name| name == CRAFTER_STRUCT);
    let source_inventory = origins.uses.finish_with(|| checkpoint(query.cancel))?;
    checkpoint(query.cancel)?;
    Ok(CrafterHostSnapshot {
        build: query.archive.build().cloned(),
        availability,
        host_state,
        rows,
        source_inventory,
        loadout_coverage,
        crafter_coverage,
        dependencies,
    })
}

fn mounts<'source>(
    loadout: &Loadout,
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    cancel: Option<&CancelToken>,
    coverage: &mut CatalogRecordCoverage,
) -> Result<Vec<CrafterHost>, CatalogError> {
    let mut admits = crafter_admission(forge, cancel)?;
    let mut cache = HashMap::new();
    let mut trees = TreeBuilder::new();
    crafter_host_projection::project_loadout(loadout, input.origins(), cancel, |index| {
        if let Some(&present) = cache.get(&index) {
            return Ok(present);
        }
        let record = forge
            .record(index)
            .ok_or(ExportAccountingError::MissingPhysicalReferenceTarget { index })?;
        let present = if admits(&record, &mut trees, coverage)?
            && let Some(owner) = read_owner(&mut trees, &record, coverage)?
        {
            read_station(&owner, input, cancel, coverage)?.is_some()
        } else {
            false
        };
        cache.insert(index, present);
        Ok(present)
    })
}
