// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use p4kx_extraction::{CancelToken, CanonicalReadObserver, DataForge, Guid, Record, TreeBuilder};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::industry_modules::blueprint_pool_definition;
use crate::entity_selection::Selected;
use crate::record_observation::RecordObservation;

#[derive(Clone, Copy)]
enum PoolState {
    Outside,
    Failed,
    Materialized,
}

pub(super) struct PoolAdmission<'read, 'source> {
    forge: &'source DataForge<'source>,
    input: &'read OriginalCatalogInput<'read, 'source>,
    cancel: Option<&'read CancelToken>,
    trees: TreeBuilder<'source>,
    states: HashMap<u32, PoolState>,
    checked_guids: HashSet<Guid>,
    pub(super) wanted: HashSet<Guid>,
    pub(super) coverage: CatalogRecordCoverage,
    pub(super) body_reads: usize,
}

impl<'read, 'source> PoolAdmission<'read, 'source> {
    pub(super) fn new(
        forge: &'source DataForge<'source>,
        input: &'read OriginalCatalogInput<'read, 'source>,
        cancel: Option<&'read CancelToken>,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
    ) -> Self {
        let mut trees = TreeBuilder::new();
        trees.set_read_observer(observer);
        Self {
            forge,
            input,
            cancel,
            trees,
            states: HashMap::new(),
            checked_guids: HashSet::new(),
            wanted: HashSet::new(),
            coverage: CatalogRecordCoverage::default(),
            body_reads: 0,
        }
    }

    pub(super) fn select_targets(
        &mut self,
        selection: CatalogRecordSelection,
    ) -> Result<HashSet<u32>, CatalogError> {
        let mut targets = HashSet::new();
        let mut guids = HashSet::new();
        let forge = self.forge;
        selection.for_each(forge, self.cancel, |record| {
            if !matches!(self.inspect(record)?, PoolState::Outside) {
                targets.insert(record.index());
                guids.insert(record.guid());
            }
            Ok(())
        })?;
        for guid in guids {
            self.ensure_guid(guid)?;
        }
        Ok(targets)
    }

    pub(super) fn ensure_guid(&mut self, guid: Guid) -> Result<(), CatalogError> {
        checkpoint(self.cancel)?;
        if self.checked_guids.contains(&guid) {
            return Ok(());
        }
        let forge = self.forge;
        for record in forge.records_by_guid(guid) {
            self.inspect(&record)?;
        }
        self.checked_guids.insert(guid);
        Ok(())
    }

    fn inspect(&mut self, record: &Record<'source>) -> Result<PoolState, CatalogError> {
        checkpoint(self.cancel)?;
        if let Some(state) = self.states.get(&record.index()) {
            return Ok(*state);
        }
        if record
            .record_path()
            .is_ok_and(|path| !crate::Kind::BlueprintPools.records().matches_path(path))
        {
            return Ok(PoolState::Outside);
        }
        let state = match crate::crafting::ledger::pool_record(record) {
            Ok(Some(Selected::Entity(_))) => self.read(record)?,
            Ok(Some(Selected::Leftover(_))) => {
                self.coverage.skipped_leftovers += 1;
                PoolState::Outside
            }
            Ok(None) => return Ok(PoolState::Outside),
            Err(_) => {
                self.coverage.unreadable_headers.push(record.index());
                PoolState::Outside
            }
        };
        self.states.insert(record.index(), state);
        Ok(state)
    }

    fn read(&mut self, record: &Record<'source>) -> Result<PoolState, CatalogError> {
        checkpoint(self.cancel)?;
        self.body_reads += 1;
        let Ok(tree) = self.trees.build(record) else {
            self.coverage.unreadable_bodies.push(record.index());
            return Ok(PoolState::Failed);
        };
        checkpoint(self.cancel)?;
        let admission = RecordObservation::capture(self.input, record)?;
        let read = blueprint_pool_definition::read(&tree, self.input, &admission, self.cancel)?;
        if !read.complete
            || read
                .notes
                .iter()
                .any(|note| note.description().issue.blocks_projection())
        {
            self.coverage.incomplete_fields.push(record.index());
        }
        self.wanted.insert(record.guid());
        Ok(PoolState::Materialized)
    }
}
