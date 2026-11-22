// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Build, CancelToken, DataForge, DataForgeEntry, Filter};

use super::read::Query;
use crate::catalog_queries::unlocalized_record_query::plan::Plan;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    ParsedCatalogQueries,
};
use crate::entity_selection::{self, Selected};
use crate::ship::ShipIdentity;
use crate::ship_store_slugs::StoreSlugs;
use crate::{CatalogError, Kind, StoreSlugSummary};

pub use crate::contract_store_slugs::ShipStoreSlug;

/// Curated Store slugs resolved for the selected canonical Ship scope.
#[derive(Debug)]
#[must_use]
pub struct ShipStoreSlugSnapshot<'mapping> {
    build: Option<Build>,
    rows: Vec<ShipStoreSlug<'mapping>>,
    summary: StoreSlugSummary,
    filter: Option<&'mapping str>,
    coverage: CatalogRecordCoverage,
}

impl<'mapping> ShipStoreSlugSnapshot<'mapping> {
    /// Borrows the Build in which Ship identities were resolved.
    #[must_use]
    pub const fn build(&self) -> Option<&Build> {
        self.build.as_ref()
    }

    /// Borrows the canonical Store slug rows in slug order.
    #[must_use]
    pub fn rows(&self) -> &[ShipStoreSlug<'mapping>] {
        &self.rows
    }

    /// Borrows curated mapping counts and unfiltered stale findings.
    #[must_use]
    pub const fn summary(&self) -> &StoreSlugSummary {
        &self.summary
    }

    /// Returns the Catalog Filter text, or none for the complete selection.
    #[must_use]
    pub const fn filter(&self) -> Option<&'mapping str> {
        self.filter
    }

    /// Borrows the read coverage of the demanded physical Ship candidates.
    #[must_use]
    pub const fn coverage(&self) -> &CatalogRecordCoverage {
        &self.coverage
    }
}

/// Supplies the curated Store slugs for the selected canonical Ship scope.
///
/// # Errors
/// Returns original-read, source-evidence, Archive-change or cancellation errors.
pub fn with_ship_store_slugs<T>(
    archive: &Archive,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ShipStoreSlugSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_ship_store_slugs(filter, cancel, inspect)
}

/// Supplies the same Store slugs from a decoded Entry bound to the Archive.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_ship_store_slugs_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ShipStoreSlugSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_ship_store_slugs(filter, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies Store slugs using this session's original DataForge.
    ///
    /// # Errors
    /// Returns original-read, source-evidence, Archive-change or cancellation errors.
    pub fn with_ship_store_slugs<T>(
        &self,
        filter: Option<&Filter>,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ShipStoreSlugSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_ship_store_slugs(filter, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies Store slugs using this reusable parsed DataForge view.
    ///
    /// # Errors
    /// Returns original-read, source-evidence, Archive-change or cancellation errors.
    pub fn with_ship_store_slugs<T>(
        &self,
        filter: Option<&Filter>,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ShipStoreSlugSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.with_store_mapping(&StoreSlugs::embedded()?, filter, cancel, inspect)
    }

    fn with_store_mapping<T>(
        &self,
        mapping: &StoreSlugs,
        filter: Option<&Filter>,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ShipStoreSlugSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            let plan = plan(forge, mapping, filter, cancel)?;
            let query = Query {
                archive,
                entry,
                forge,
                selection: CatalogRecordSelection::All,
                languages: &[],
                cancel,
            };
            query.run_plan(plan, |snapshot| {
                let ships = snapshot
                    .rows()
                    .iter()
                    .map(|ship| {
                        let identity = ShipIdentity::read(ship.source())?;
                        Ok((identity.guid.0, identity.class_name))
                    })
                    .collect::<Result<Vec<_>, CatalogError>>()?;
                let resolved = mapping.resolve(ships, filter.is_some());
                Ok(inspect(ShipStoreSlugSnapshot {
                    build: snapshot.build,
                    rows: resolved.exported,
                    summary: resolved.summary,
                    filter: filter.map(Filter::as_str),
                    coverage: snapshot.coverage,
                }))
            })?
        })
    }
}

fn plan<'source>(
    forge: &'source DataForge<'source>,
    mapping: &StoreSlugs,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<Plan<'source>, CatalogError> {
    let guids = mapping.ship_guids(filter.is_some());
    Plan::read_with(
        forge,
        CatalogRecordSelection::All,
        cancel,
        None,
        |record, _, coverage| {
            if !guids.contains(&record.guid()) {
                return Ok(false);
            }
            match entity_selection::select(record, filter) {
                Ok(Some(Selected::Entity(Kind::Ships))) => return Ok(true),
                Ok(Some(Selected::Leftover(Kind::Ships))) => coverage.skipped_leftovers += 1,
                Ok(_) => {}
                Err(_) => coverage.unreadable_headers.push(record.index()),
            }
            Ok(false)
        },
    )
}

#[cfg(test)]
#[path = "equipment_ship_store_slug_tests.rs"]
mod tests;
