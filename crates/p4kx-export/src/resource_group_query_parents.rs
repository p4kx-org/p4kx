// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use p4kx_extraction::{CancelToken, CanonicalReadObserver, Record, RecordTree, TreeBuilder};

use super::request::Query;
use super::snapshot::ResourceGroupParentSnapshot;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_queries::unlocalized_record_query::plan::Plan;
use crate::catalog_query_session::checkpoint;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::resource_group_catalog;
use crate::entity_selection::{self, Selected};
use crate::guid_text::CatalogGuid;
use crate::resource_group::{ParentSource, Parents};
use crate::{CatalogError, Kind};

#[derive(Default)]
pub(super) struct Observers {
    pub(super) owners: Option<Arc<dyn CanonicalReadObserver>>,
    pub(super) groups: Option<Arc<dyn CanonicalReadObserver>>,
}

struct Reused<'read, 'source> {
    ready: HashMap<u32, &'read RecordTree<'source>>,
    failed: HashSet<u32>,
}

impl<'read, 'source> Reused<'read, 'source> {
    fn new(plan: &'read Plan<'source>) -> Self {
        Self {
            ready: plan
                .owners
                .iter()
                .map(|owner| (owner.record.index(), &owner.tree))
                .collect(),
            failed: plan.coverage.unreadable_bodies.iter().copied().collect(),
        }
    }
}

#[derive(Default)]
struct Survey {
    index: Parents,
    coverage: CatalogRecordCoverage,
}

impl Query<'_> {
    pub(super) fn run_parents<T>(
        self,
        inspect: impl FnOnce(ResourceGroupParentSnapshot) -> T,
    ) -> Result<T, CatalogError> {
        self.run_parents_observed(Observers::default(), inspect)
    }

    pub(super) fn run_parents_observed<T>(
        self,
        observers: Observers,
        inspect: impl FnOnce(ResourceGroupParentSnapshot) -> T,
    ) -> Result<T, CatalogError> {
        let plan = Plan::read(
            self.forge,
            Kind::ResourceGroups,
            self.selection,
            self.cancel,
            observers.owners,
        )?;
        let origins = CatalogOrigins::for_archive_records(self.archive, self.entry)?;
        let input = OriginalCatalogInput::new(self.forge, &origins);
        let survey = if plan.owners.is_empty() {
            Survey::default()
        } else {
            Survey::read(&input, &Reused::new(&plan), observers.groups, self.cancel)?
        };
        let mut rows = Vec::with_capacity(plan.owners.len());
        for owner in plan.owners {
            checkpoint(self.cancel)?;
            let guid = CatalogGuid(owner.record.guid());
            let choice = survey.index.choice(&guid.0, self.cancel)?;
            let row = resource_group_catalog::parent(&origins, owner.record.index(), guid, choice)?;
            rows.push((guid.text(), owner.record.index(), row));
        }
        rows.sort_by_key(|(guid, physical, _)| (*guid, *physical));
        let table = CatalogTable::new(
            self.forge,
            &[Kind::ResourceGroups.records().type_name],
            rows.into_iter().map(|(_, _, row)| row).collect(),
        );
        let source_inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        Ok(inspect(ResourceGroupParentSnapshot {
            build: self.archive.build().cloned(),
            availability: table.availability,
            rows: table.rows,
            source_inventory,
            coverage: plan.coverage,
            dependency_coverage: survey.coverage,
        }))
    }
}

impl Survey {
    fn read(
        input: &OriginalCatalogInput<'_, '_>,
        reused: &Reused<'_, '_>,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let mut result = Self::default();
        let mut trees = TreeBuilder::new();
        trees.set_read_observer(observer);
        for record in input.forge().records() {
            checkpoint(cancel)?;
            match entity_selection::select(&record, None) {
                Ok(Some(Selected::Entity(Kind::ResourceGroups))) => {}
                Ok(Some(Selected::Leftover(Kind::ResourceGroups))) => {
                    result.coverage.skipped_leftovers += 1;
                    continue;
                }
                Ok(_) => continue,
                Err(_) => {
                    if let Some(source) = ParentSource::metadata_failure(&record, input.origins())?
                    {
                        result.coverage.unreadable_headers.push(record.index());
                        result.index.failed(source);
                    }
                    continue;
                }
            }
            if let Some(tree) = reused.ready.get(&record.index()) {
                result.add_tree(&record, tree, input, cancel)?;
            } else if reused.failed.contains(&record.index()) {
                result.add_failure(&record, input)?;
            } else if let Ok(tree) = trees.build(&record) {
                result.add_tree(&record, &tree, input, cancel)?;
            } else {
                result.add_failure(&record, input)?;
            }
        }
        Ok(result)
    }

    fn add_tree(
        &mut self,
        record: &Record<'_>,
        tree: &RecordTree<'_>,
        input: &OriginalCatalogInput<'_, '_>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        input.record_scope(record, tree)?;
        let (source, children) = ParentSource::read(tree, input.origins(), cancel)?;
        if !source.is_complete() {
            self.coverage.incomplete_fields.push(record.index());
        }
        self.index.add(source, children, cancel)
    }

    fn add_failure(
        &mut self,
        record: &Record<'_>,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), CatalogError> {
        self.coverage.unreadable_bodies.push(record.index());
        self.index
            .failed(ParentSource::canonical_failure(record, input.origins())?);
        Ok(())
    }
}
