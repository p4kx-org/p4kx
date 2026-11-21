// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{Archive, CancelToken, CanonicalReadObserver, DataForge, DataForgeEntry};

use super::BlueprintPoolSnapshot;
use crate::CatalogError;
use crate::Kind;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_queries::CatalogRecordSelection;
use crate::catalog_queries::unlocalized_record_query::plan::Plan;
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::industry_modules::{blueprint_pool_definition, blueprint_pool_tables};
use crate::record_observation::RecordObservation;

pub(super) fn with_parsed<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    observer: Option<Arc<dyn CanonicalReadObserver>>,
    inspect: impl FnOnce(BlueprintPoolSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    Query {
        archive,
        entry,
        forge,
        selection,
        cancel,
    }
    .run(observer, inspect)
}

pub(super) struct Query<'query> {
    pub(super) archive: &'query Archive,
    pub(super) entry: &'query DataForgeEntry,
    pub(super) forge: &'query DataForge<'query>,
    pub(super) selection: CatalogRecordSelection,
    pub(super) cancel: Option<&'query CancelToken>,
}

impl Query<'_> {
    pub(super) fn run<T>(
        self,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
        inspect: impl FnOnce(BlueprintPoolSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        let mut plan = Plan::read(
            self.forge,
            Kind::BlueprintPools,
            self.selection,
            self.cancel,
            observer,
        )?;
        let origins = CatalogOrigins::for_archive_records(self.archive, self.entry)?;
        let input = OriginalCatalogInput::new(self.forge, &origins);
        let mut rows = Vec::new();
        for owner in plan.owners {
            checkpoint(self.cancel)?;
            let admission = RecordObservation::capture(&input, &owner.record)?;
            let read =
                blueprint_pool_definition::read(&owner.tree, &input, &admission, self.cancel)?;
            if !read.complete
                || read
                    .notes
                    .iter()
                    .any(|note| note.description().issue.blocks_projection())
            {
                plan.coverage.incomplete_fields.push(owner.record.index());
            }
            rows.push(read.prepared);
        }
        let tables = blueprint_pool_tables::tables(self.forge, rows, Vec::new());
        let source_inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        Ok(inspect(BlueprintPoolSnapshot {
            build: self.archive.build().cloned(),
            availability: tables.definitions.availability,
            rows: tables.definitions.rows,
            weights: tables.weights.rows,
            source_inventory,
            coverage: plan.coverage,
        }))
    }
}
