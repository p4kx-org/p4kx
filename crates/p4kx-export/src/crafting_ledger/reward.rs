// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_extraction::{CancelToken, Guid, Record, TreeBuilder};
use rayon::prelude::*;

use crate::crafting_sources;
use crate::crafting_types::{BlueprintPool, RewardSource};
use crate::export_ledger::ExportLedger;
use crate::{CatalogError, CatalogPhase, CatalogProgress, Kind};

const PROGRESS_STEP: usize = 1024;

/// Actual reward-source search results and source-reading observations.
pub(crate) struct RewardSearch<'f> {
    pub(crate) sources: Vec<RewardSource<'f>>,
    pub(crate) unreadable: usize,
    pub(crate) ledger: ExportLedger,
}

enum Scanned<'f> {
    Read {
        type_name: &'f str,
        sources: Vec<RewardSource<'f>>,
        ledger: ExportLedger,
    },
    Failed {
        type_name: Option<&'f str>,
        ledger: ExportLedger,
    },
}

fn scan<'f>(
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    wanted: &HashSet<Guid>,
    cancel: Option<&CancelToken>,
) -> Result<Scanned<'f>, CatalogError> {
    let read = crate::export_dependency_read::read(trees, record, Kind::BlueprintPools, cancel)?;
    let Ok(tree) = read.tree else {
        return Ok(Scanned::Failed {
            type_name: record.type_name().ok(),
            ledger: read.ledger,
        });
    };
    let mut sources = Vec::new();
    crafting_sources::collect(&tree, wanted, &mut sources);
    Ok(Scanned::Read {
        type_name: tree.type_name(),
        sources,
        ledger: read.ledger,
    })
}

fn gather(scanned: Vec<Scanned<'_>>) -> RewardSearch<'_> {
    let mut sources = Vec::new();
    let mut holding = HashSet::new();
    let mut unreadable = Vec::new();
    let mut ledger = ExportLedger::default();
    for scanned in scanned {
        match scanned {
            Scanned::Read {
                type_name,
                sources: found,
                ledger: observed,
            } => {
                ledger.merge(observed);
                if !found.is_empty() {
                    holding.insert(type_name);
                }
                sources.extend(found);
            }
            Scanned::Failed {
                type_name,
                ledger: observed,
            } => {
                ledger.merge(observed);
                unreadable.push(type_name);
            }
        }
    }
    let unreadable = if sources.is_empty() {
        unreadable.len()
    } else {
        unreadable
            .into_iter()
            .filter(|name| name.is_some_and(|name| !name.is_empty() && holding.contains(name)))
            .count()
    };
    RewardSearch {
        sources,
        unreadable,
        ledger,
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
    let done = AtomicUsize::new(0);
    let scanned = records
        .par_iter()
        .map_init(TreeBuilder::new, |trees, record| {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            let scanned = scan(trees, record, &wanted, cancel)?;
            tick(&done, records.len(), progress);
            Ok(scanned)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(gather(scanned))
}

#[cfg(test)]
#[path = "reward_tests.rs"]
mod tests;
