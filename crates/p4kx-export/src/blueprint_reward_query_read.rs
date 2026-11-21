// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_extraction::{
    Archive, CancelToken, CanonicalReadObserver, DataForge, DataForgeEntry, Record, RecordTree,
    TreeBuilder,
};

use super::{BlueprintRewardSelection, BlueprintRewardSnapshot, BlueprintRewardSource};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::industry_modules::{blueprint_pool_tables, blueprint_reward_source};
use crate::export_diagnostic::ExportDiagnosticSeverity;
use crate::export_ledger::ExportLedger;
use crate::record_observation::RecordObservation;

#[path = "blueprint_reward_query_pools.rs"]
mod pools;
use pools::PoolAdmission;

pub(super) fn with_parsed<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    selection: BlueprintRewardSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(BlueprintRewardSnapshot<'_>) -> T,
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
    pub(super) entry: &'query DataForgeEntry,
    pub(super) forge: &'query DataForge<'query>,
    pub(super) selection: BlueprintRewardSelection,
    pub(super) cancel: Option<&'query CancelToken>,
}

impl Query<'_> {
    pub(super) fn run<T>(
        self,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
        inspect: impl FnOnce(BlueprintRewardSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        let origins = CatalogOrigins::for_archive_records(self.archive, self.entry)?;
        let input = OriginalCatalogInput::new(self.forge, &origins);
        let mut trees = TreeBuilder::new();
        trees.set_read_observer(observer.clone());
        let mut survey = Survey {
            input: &input,
            pools: PoolAdmission::new(self.forge, &input, self.cancel, observer),
            coverage: CatalogRecordCoverage::default(),
            rows: Vec::new(),
            records_read: 0,
            cancel: self.cancel,
        };
        match self.selection {
            BlueprintRewardSelection::SourceOwners(selection) => {
                selection.for_each(self.forge, self.cancel, |record| {
                    survey.read(&mut trees, record, None)
                })?;
            }
            BlueprintRewardSelection::TargetPools(selection) => {
                let targets = survey.pools.select_targets(selection)?;
                if !targets.is_empty() {
                    CatalogRecordSelection::All.for_each(self.forge, self.cancel, |record| {
                        survey.read(&mut trees, record, Some(&targets))
                    })?;
                }
            }
        }
        let table = blueprint_pool_tables::tables(self.forge, Vec::new(), survey.rows).rewards;
        let source_inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        Ok(inspect(BlueprintRewardSnapshot {
            build: self.archive.build().cloned(),
            availability: table.availability,
            selection: self.selection,
            rows: table.rows,
            source_inventory,
            source_coverage: survey.coverage,
            pool_coverage: survey.pools.coverage,
            source_records_read: survey.records_read,
            pool_records_read: survey.pools.body_reads,
        }))
    }
}

struct Survey<'read, 'source> {
    input: &'read OriginalCatalogInput<'read, 'source>,
    pools: PoolAdmission<'read, 'source>,
    coverage: CatalogRecordCoverage,
    rows: Vec<BlueprintRewardSource<'source>>,
    records_read: usize,
    cancel: Option<&'read CancelToken>,
}

impl<'source> Survey<'_, 'source> {
    fn read(
        &mut self,
        trees: &mut TreeBuilder<'source>,
        record: &Record<'source>,
        targets: Option<&HashSet<u32>>,
    ) -> Result<(), CatalogError> {
        self.records_read += 1;
        if record.type_name().is_err() || record.record_path().is_err() || record.name().is_err() {
            self.coverage.unreadable_headers.push(record.index());
        }
        crate::crafting_scan::with_reward_record(trees, record, None, self.cancel, |_, read| {
            let Ok(tree) = read.tree else {
                self.coverage.unreadable_bodies.push(record.index());
                return Ok(());
            };
            let projection_incomplete = self.project(record, &tree, targets)?;
            if incomplete(&read.ledger) || projection_incomplete {
                self.coverage.incomplete_fields.push(record.index());
            }
            Ok(())
        })
    }

    fn project(
        &mut self,
        record: &Record<'source>,
        tree: &RecordTree<'source>,
        targets: Option<&HashSet<u32>>,
    ) -> Result<bool, CatalogError> {
        if !self.matches(tree, targets)? {
            return Ok(false);
        }
        let admission = RecordObservation::capture(self.input, record)?;
        let found = blueprint_reward_source::read_selected(
            tree,
            self.input,
            &admission,
            &self.pools.wanted,
            targets,
            self.cancel,
        )?;
        let incomplete = found
            .notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
            || found.rows.iter().any(super::reward_coverage::incomplete);
        self.rows.extend(found.rows);
        Ok(incomplete)
    }

    fn matches(
        &mut self,
        tree: &RecordTree<'source>,
        targets: Option<&HashSet<u32>>,
    ) -> Result<bool, CatalogError> {
        let mut found = false;
        for step in crate::crafting_sources::walk(tree) {
            checkpoint(self.cancel)?;
            let Some(reference) = step.pool_reference(tree) else {
                continue;
            };
            let guid = reference.guid();
            if targets.is_none() {
                self.pools.ensure_guid(guid)?;
            }
            if self.pools.wanted.contains(&guid)
                && targets.is_none_or(|targets| targets.contains(&reference.record_index()))
            {
                found = true;
                if targets.is_some() {
                    break;
                }
            }
        }
        Ok(found)
    }
}

fn incomplete(ledger: &ExportLedger) -> bool {
    ledger.field_inspections.iter().any(|inspection| {
        !inspection.complete
            || inspection
                .findings
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.severity == ExportDiagnosticSeverity::Error)
    }) || ledger
        .field_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ExportDiagnosticSeverity::Error)
}
