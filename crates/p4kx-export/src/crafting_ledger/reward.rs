// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_extraction::{CancelToken, Guid, Record, TreeBuilder};
use rayon::iter::ParallelIterator;
use rayon::slice::ParallelSlice;

use crate::build::original_input::OriginalCatalogInput;
use crate::crafting_sources;
use crate::crafting_types::{BlueprintPool, RewardSource};
use crate::export_ledger::ExportLedger;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, CatalogPhase, CatalogProgress, Kind};

const PROGRESS_STEP: usize = 1024;
// Several contiguous ranges per worker allow stealing when Record sizes vary.
const PARTITIONS_PER_THREAD: usize = 4;

/// Actual reward-source search results and source-reading observations.
pub(crate) struct RewardSearch<'f> {
    pub(crate) sources: Vec<RewardSource<'f>>,
    pub(crate) declarations: Vec<
        crate::catalog_v3::industry_modules::blueprint_reward_source::BlueprintRewardSource<'f>,
    >,
    pub(crate) numbers: Vec<crate::catalog_number_capture::CapturedNumber>,
    pub(crate) unreadable: usize,
    pub(crate) ledger: ExportLedger,
}

#[derive(Default)]
struct RewardAccumulator<'f> {
    sources: Vec<RewardSource<'f>>,
    declarations: Vec<
        crate::catalog_v3::industry_modules::blueprint_reward_source::BlueprintRewardSource<'f>,
    >,
    numbers: Vec<crate::catalog_number_capture::CapturedNumber>,
    holding: HashSet<&'f str>,
    failures_by_type: HashMap<&'f str, usize>,
    failed: usize,
    ledger: ExportLedger,
}

impl<'f> RewardAccumulator<'f> {
    fn scan(
        &mut self,
        trees: &mut TreeBuilder<'f>,
        record: &Record<'f>,
        wanted: &HashSet<Guid>,
        input: Option<&OriginalCatalogInput<'_, 'f>>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        crate::crafting_scan::with_reward_record(trees, record, input, cancel, |admission, read| {
            self.ledger.merge(read.ledger);
            self.collect_read(record, wanted, input, cancel, admission, read.tree)
        })
    }

    fn collect_read(
        &mut self,
        record: &Record<'f>,
        wanted: &HashSet<Guid>,
        input: Option<&OriginalCatalogInput<'_, 'f>>,
        cancel: Option<&CancelToken>,
        admission: &RecordObservation<'_, 'f>,
        tree: Result<p4kx_extraction::RecordTree<'f>, p4kx_extraction::DataForgeError>,
    ) -> Result<(), CatalogError> {
        match tree {
            Ok(tree) => {
                if let Some(input) = input {
                    let found = crate::catalog_v3::industry_modules::blueprint_reward_source::read(
                        &tree, input, admission, wanted, cancel,
                    )?;
                    crate::source_projection_notes::record(
                        Kind::BlueprintPools,
                        crate::export_diagnostic::ExportSourceRole::Dependency,
                        admission,
                        &tree,
                        found.notes,
                        &mut self.ledger,
                    )?;
                    self.collect_placements(tree.type_name(), |sources| {
                        sources.extend(found.interpreted);
                    });
                    self.declarations.extend(found.rows);
                    self.numbers.extend(found.numbers);
                } else {
                    self.collect_placements(tree.type_name(), |sources| {
                        crafting_sources::collect(&tree, wanted, sources);
                    });
                }
            }
            Err(_) => self.fail(record.type_name().ok()),
        }
        Ok(())
    }

    fn collect_placements(
        &mut self,
        type_name: &'f str,
        collect: impl FnOnce(&mut Vec<RewardSource<'f>>),
    ) {
        let previous = self.sources.len();
        collect(&mut self.sources);
        if self.sources.len() > previous {
            self.holding.insert(type_name);
        }
    }

    fn fail(&mut self, type_name: Option<&'f str>) {
        self.failed += 1;
        if let Some(name) = type_name.filter(|name| !name.is_empty()) {
            *self.failures_by_type.entry(name).or_default() += 1;
        }
    }

    fn append(&mut self, mut next: Self) {
        self.sources.append(&mut next.sources);
        self.declarations.append(&mut next.declarations);
        self.numbers.append(&mut next.numbers);
        self.holding.extend(next.holding);
        for (name, count) in next.failures_by_type {
            *self.failures_by_type.entry(name).or_default() += count;
        }
        self.failed += next.failed;
        self.ledger.merge(next.ledger);
    }

    fn finish(self) -> RewardSearch<'f> {
        let unreadable = if self.sources.is_empty() {
            self.failed
        } else {
            self.holding
                .iter()
                .filter_map(|name| self.failures_by_type.get(name))
                .sum()
        };
        RewardSearch {
            sources: self.sources,
            declarations: self.declarations,
            numbers: self.numbers,
            unreadable,
            ledger: self.ledger,
        }
    }
}

fn tick(done: &AtomicUsize, total: usize, progress: Option<&(dyn Fn(CatalogProgress) + Sync)>) {
    let finished = done.fetch_add(1, Ordering::Relaxed).saturating_add(1);
    if let Some(progress) = progress
        && (finished.is_multiple_of(PROGRESS_STEP) || finished == total)
    {
        progress(CatalogProgress {
            phase: CatalogPhase::Crafting,
            records_done: finished,
            records_total: total,
        });
    }
}

/// Finds pool rewards while retaining every actual dependency-read outcome.
pub(crate) fn reward_sources<'f>(
    records: &[Record<'f>],
    pools: &[BlueprintPool<'f>],
    cancel: Option<&CancelToken>,
    progress: Option<&(dyn Fn(CatalogProgress) + Sync)>,
) -> Result<RewardSearch<'f>, CatalogError> {
    let wanted = pools.iter().map(|pool| pool.guid).collect();
    let partitions = rayon::current_num_threads().saturating_mul(PARTITIONS_PER_THREAD);
    let chunk_size = records.len().div_ceil(partitions).max(1);
    scan_chunks(records, chunk_size, &wanted, None, cancel, progress)
}

pub(crate) fn reward_sources_for<'f>(
    records: &[Record<'f>],
    wanted: &HashSet<Guid>,
    input: &OriginalCatalogInput<'_, 'f>,
    cancel: Option<&CancelToken>,
    progress: Option<&(dyn Fn(CatalogProgress) + Sync)>,
) -> Result<RewardSearch<'f>, CatalogError> {
    let partitions = rayon::current_num_threads().saturating_mul(PARTITIONS_PER_THREAD);
    let chunk_size = records.len().div_ceil(partitions).max(1);
    scan_chunks(records, chunk_size, wanted, Some(input), cancel, progress)
}

fn scan_chunks<'f>(
    records: &[Record<'f>],
    chunk_size: usize,
    wanted: &HashSet<Guid>,
    input: Option<&OriginalCatalogInput<'_, 'f>>,
    cancel: Option<&CancelToken>,
    progress: Option<&(dyn Fn(CatalogProgress) + Sync)>,
) -> Result<RewardSearch<'f>, CatalogError> {
    let done = AtomicUsize::new(0);
    let chunks = records
        .par_chunks(chunk_size)
        .map(|chunk| {
            let mut trees = TreeBuilder::new();
            let mut found = RewardAccumulator::default();
            for record in chunk {
                found.scan(&mut trees, record, wanted, input, cancel)?;
                tick(&done, records.len(), progress);
            }
            Ok(found)
        })
        .collect::<Result<Vec<_>, CatalogError>>()?;
    let mut chunks = chunks.into_iter();
    let mut collected = chunks.next().unwrap_or_default();
    for chunk in chunks {
        collected.append(chunk);
    }
    Ok(collected.finish())
}

#[cfg(test)]
#[path = "reward_tests.rs"]
mod tests;
