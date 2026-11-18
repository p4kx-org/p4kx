// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{
    CancelToken, CanonicalReadObserver, DataForgeError, Guid, Record, TreeBuilder,
};

use super::snapshot::BlueprintDependencyCoverage;
use crate::blueprint_catalog_rows::ProcessChoice;
use crate::blueprint_rule_inputs::BlueprintRules;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_query_session::checkpoint;
use crate::crafting_records;
use crate::record_observation::RecordObservation;
use crate::selection::{self, KindRule};
use crate::{CatalogError, ExportAccountingError};

pub(super) struct Dependencies {
    pub(super) coverage: BlueprintDependencyCoverage,
    processes: Option<Processes>,
    observer: Option<Arc<dyn CanonicalReadObserver>>,
}

#[derive(Default)]
struct Processes {
    rules: BlueprintRules,
    guid: Option<Guid>,
    count: usize,
}

impl Dependencies {
    pub(super) fn new(observer: Option<Arc<dyn CanonicalReadObserver>>) -> Self {
        Self {
            coverage: BlueprintDependencyCoverage::default(),
            processes: None,
            observer,
        }
    }

    pub(super) fn globals(
        &mut self,
        input: &OriginalCatalogInput<'_, '_>,
        cancel: Option<&CancelToken>,
    ) -> Result<BlueprintRules, CatalogError> {
        let mut result = BlueprintRules::default();
        let mut trees = TreeBuilder::new();
        trees.set_read_observer(self.observer.clone());
        for record in input.forge().records() {
            checkpoint(cancel)?;
            match matches(&record, crafting_records::GLOBALS) {
                Ok(false) => continue,
                Ok(true) => {}
                Err(_) => {
                    result.merge(BlueprintRules::failed_header(&record));
                    self.coverage
                        .globals
                        .unreadable_headers
                        .push(record.index());
                    continue;
                }
            }
            let admission = RecordObservation::capture(input, &record)?;
            if let Ok(tree) = trees.build(&record) {
                result.merge(BlueprintRules::read_globals(
                    &tree, &admission, input, cancel,
                )?);
            } else {
                self.coverage.globals.unreadable_bodies.push(record.index());
                result.defaults.unavailable = true;
                result.item_exclusions.unavailable = true;
            }
        }
        Ok(result)
    }

    pub(super) fn process<'rules>(
        &'rules mut self,
        input: &OriginalCatalogInput<'_, '_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ProcessChoice<'rules>, CatalogError> {
        if self.processes.is_none() {
            self.processes = Some(read_processes(
                input,
                &mut self.coverage.dismantle,
                self.observer.clone(),
                cancel,
            )?);
        }
        self.processes.as_ref().map_or_else(
            || Err(ExportAccountingError::ConflictingSourceDatum.into()),
            |processes| {
                Ok(ProcessChoice {
                    guid: processes.guid,
                    sources: &processes.rules.processes,
                    unavailable: processes.rules.process_unavailable,
                })
            },
        )
    }
}

fn read_processes(
    input: &OriginalCatalogInput<'_, '_>,
    coverage: &mut CatalogRecordCoverage,
    observer: Option<Arc<dyn CanonicalReadObserver>>,
    cancel: Option<&CancelToken>,
) -> Result<Processes, CatalogError> {
    let mut result = Processes::default();
    let mut trees = TreeBuilder::new();
    trees.set_read_observer(observer);
    for record in input.forge().records() {
        checkpoint(cancel)?;
        match matches(&record, crafting_records::DISMANTLES) {
            Ok(false) => continue,
            Ok(true) => {}
            Err(_) => {
                result.rules.merge(BlueprintRules::failed_header(&record));
                coverage.unreadable_headers.push(record.index());
                continue;
            }
        }
        let admission = RecordObservation::capture(input, &record)?;
        let Ok(tree) = trees.build(&record) else {
            coverage.unreadable_bodies.push(record.index());
            result.rules.process_unavailable = true;
            continue;
        };
        result.rules.merge(BlueprintRules::read_dismantle(
            &tree, &admission, input, cancel,
        )?);
        if selection::is_leftover(tree.record_path(), crate::record_fields::class_name(&tree)) {
            coverage.skipped_leftovers += 1;
            continue;
        }
        if let Some(process) = crafting_records::dismantle(&tree) {
            result.count = result
                .count
                .checked_add(1)
                .ok_or(ExportAccountingError::CounterOverflow)?;
            result.guid = (result.count == 1).then_some(process.guid);
        }
    }
    Ok(result)
}

fn matches(record: &Record<'_>, rule: KindRule) -> Result<bool, DataForgeError> {
    let path = record.record_path()?;
    if !rule.matches_path(path) {
        return Ok(false);
    }
    Ok(rule.selects(path, record.type_name()?))
}
