// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, Record, RecordTree, SourceInventory,
};

use crate::CatalogError;
use crate::Kind;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::unlocalized_record_query::plan::Plan;
use crate::catalog_queries::{
    CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection, DatasetAvailability,
    ParsedCatalogQueries,
};
use crate::catalog_query_session::{CatalogQuerySource, checkpoint};
use crate::catalog_table::CatalogTable;
use crate::contract_loot_legacy::{LegacyLootArchetype, LegacyLootTable};
use crate::guid_text::CatalogGuid;
use crate::loot_legacy::{legacy_archetype, legacy_table};

/// Scoped original legacy loot rows, physical inputs and coverage.
#[derive(Debug)]
#[non_exhaustive]
pub struct LegacyLootSnapshot<Row> {
    /// Archive Build, when identified.
    pub build: Option<Build>,
    /// Availability of the exact source family.
    pub availability: DatasetAvailability,
    /// Selected rows in Catalog order.
    pub rows: Vec<Row>,
    /// Original physical sources and exact field contributions.
    pub source_inventory: SourceInventory,
    /// Outcomes of the selected original Records.
    pub coverage: CatalogRecordCoverage,
}

impl CatalogQuerySession<'_> {
    /// Queries original older loot tables from the reusable session.
    ///
    /// # Errors
    /// Returns input, origin, change or cancellation errors.
    pub fn with_legacy_loot_tables<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(LegacyLootSnapshot<LegacyLootTable<'_>>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_legacy_loot_tables(selection, cancel, inspect)
    }

    /// Queries original older loot archetypes from the reusable session.
    ///
    /// # Errors
    /// Returns input, origin, change or cancellation errors.
    pub fn with_legacy_loot_archetypes<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(LegacyLootSnapshot<LegacyLootArchetype<'_>>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_legacy_loot_archetypes(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Queries older table Records by physical Record index, GUID or family.
    ///
    /// # Errors
    /// Returns input, origin, change or cancellation errors.
    pub fn with_legacy_loot_tables<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(LegacyLootSnapshot<LegacyLootTable<'_>>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            let query = Query {
                archive,
                entry,
                forge,
                cancel,
            };
            query
                .run(Kind::LegacyLootTables, selection, legacy_table, |row| {
                    row.original.incomplete_fields
                })
                .map(inspect)
        })
    }

    /// Queries older archetype Records by physical Record index, GUID or family.
    ///
    /// # Errors
    /// Returns input, origin, change or cancellation errors.
    pub fn with_legacy_loot_archetypes<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(LegacyLootSnapshot<LegacyLootArchetype<'_>>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            let query = Query {
                archive,
                entry,
                forge,
                cancel,
            };
            query
                .run(
                    Kind::LegacyLootArchetypes,
                    selection,
                    legacy_archetype,
                    |row| row.original.incomplete_fields,
                )
                .map(inspect)
        })
    }
}

struct Query<'read, 'source> {
    archive: &'read Archive,
    entry: CatalogQuerySource<'read>,
    forge: &'source DataForge<'source>,
    cancel: Option<&'read CancelToken>,
}

type Project<'source, Row> = fn(
    &Record<'source>,
    &RecordTree<'source>,
    &OriginalCatalogInput<'_, 'source>,
    Option<&CancelToken>,
) -> Result<Row, CatalogError>;

impl<'source> Query<'_, 'source> {
    fn run<Row>(
        &self,
        kind: Kind,
        selection: CatalogRecordSelection,
        project: Project<'source, Row>,
        incomplete: impl Fn(&Row) -> bool,
    ) -> Result<LegacyLootSnapshot<Row>, CatalogError> {
        let mut plan = Plan::read(self.forge, kind, selection, self.cancel, None)?;
        let origins = self.entry.records(self.archive)?;
        let input = OriginalCatalogInput::new(self.forge, &origins);
        let mut rows = Vec::new();
        for owner in plan.owners {
            checkpoint(self.cancel)?;
            let value = project(&owner.record, &owner.tree, &input, self.cancel)?;
            let index = owner.record.index();
            if incomplete(&value) {
                plan.coverage.incomplete_fields.push(index);
            }
            rows.push(((CatalogGuid(owner.record.guid()).text(), index), value));
        }
        rows.sort_by_key(|(key, _)| *key);
        let table = CatalogTable::new(
            self.forge,
            &[kind.records().type_name],
            rows.into_iter().map(|(_, row)| row).collect(),
        );
        let source_inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        Ok(LegacyLootSnapshot {
            build: self.archive.build().cloned(),
            availability: table.availability,
            rows: table.rows,
            source_inventory,
            coverage: plan.coverage,
        })
    }
}
