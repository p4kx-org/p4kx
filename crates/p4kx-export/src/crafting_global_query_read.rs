// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{Archive, CancelToken, CanonicalReadObserver, DataForge};

use super::CraftingGlobalSnapshot;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::CatalogRecordSelection;
use crate::catalog_queries::unlocalized_record_query::plan::Plan;
use crate::catalog_query_session::{CatalogQuerySource, checkpoint};
use crate::catalog_v3::crafting_global;
use crate::crafting_records::GLOBALS;
use crate::record_observation::RecordObservation;

pub(super) fn with_parsed<T>(
    archive: &Archive,
    entry: CatalogQuerySource<'_>,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(CraftingGlobalSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    Query {
        archive,
        entry,
        forge,
        selection,
        cancel,
    }
    .run(None, inspect)
}

pub(super) struct Query<'query> {
    pub(super) archive: &'query Archive,
    pub(super) entry: CatalogQuerySource<'query>,
    pub(super) forge: &'query DataForge<'query>,
    pub(super) selection: CatalogRecordSelection,
    pub(super) cancel: Option<&'query CancelToken>,
}

impl Query<'_> {
    pub(super) fn run<T>(
        self,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
        inspect: impl FnOnce(CraftingGlobalSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        let mut plan = Plan::read_rule(self.forge, GLOBALS, self.selection, self.cancel, observer)?;
        let origins = self.entry.records(self.archive)?;
        let input = OriginalCatalogInput::new(self.forge, &origins);
        let mut rows = Vec::new();
        for owner in plan.owners {
            checkpoint(self.cancel)?;
            let admission = RecordObservation::capture(&input, &owner.record)?;
            let read = crafting_global::read(&owner.tree, &input, &admission, self.cancel)?;
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
        let table = crafting_global::table(self.forge, rows);
        let source_inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        Ok(inspect(CraftingGlobalSnapshot {
            build: self.archive.build().cloned(),
            availability: table.availability,
            rows: table.rows,
            source_inventory,
            coverage: plan.coverage,
        }))
    }
}
