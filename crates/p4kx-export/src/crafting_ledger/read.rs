// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter, Record, TreeBuilder};
use rayon::prelude::*;

use crate::crafting_scan;
use crate::crafting_warning::CraftingWarning;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition, SourceObservation};
use crate::{CatalogError, CatalogFailure, CatalogProgress, Kind};

use super::ReadCrafting;
use super::fault::Fault;
use super::projection::{self, Piece};
use super::resource_scope::ResourceLinkScope;
use super::selection::{self, ReadPolicy, Selection};

pub(super) struct Tracked<'f> {
    pub(super) piece: Option<Piece<'f>>,
    pub(super) ledger: ExportLedger,
}

fn candidate<'f>(
    record: &Record<'f>,
    error: &p4kx_extraction::DataForgeError,
    filter: Option<&Filter>,
) -> Tracked<'f> {
    let source = selection::candidate_kind(record).map_or_else(
        || {
            SourceObservation::catalog_source(
                record,
                ExportSourceRole::SelectionCandidate,
                SourceDisposition::ReadFailed,
            )
        },
        |kind| {
            SourceObservation::new(
                kind,
                record,
                ExportSourceRole::SelectionCandidate,
                SourceDisposition::ReadFailed,
            )
        },
    );
    let mut ledger = ExportLedger::default();
    ledger.observe(source);
    Tracked {
        piece: (!selection::outside(record, filter))
            .then(|| Piece::Failed(CatalogFailure::of(record, error))),
        ledger,
    }
}

fn failure<'f>(
    selected: Selection,
    record: &Record<'f>,
    fault: Fault<'_>,
    mut ledger: ExportLedger,
    filter: Option<&Filter>,
) -> Tracked<'f> {
    let mut source = selected
        .observation(record, fault.disposition())
        .diagnosed(fault.diagnosis());
    if selected.policy == ReadPolicy::SkipAfter {
        ledger.observe(selected.observation(record, SourceDisposition::SkippedBeforeRead));
        source.role = ExportSourceRole::Dependency;
        ledger.reassign_inspection_role(
            selected.action.kind(),
            record.index(),
            selected.role,
            ExportSourceRole::Dependency,
        );
    }
    ledger.observe(source);
    Tracked {
        piece: fault
            .compatibility_failure(record, filter)
            .then(|| Piece::Failed(CatalogFailure::of(record, fault.error()))),
        ledger,
    }
}

fn one<'f>(
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    scope: ResourceLinkScope,
) -> Result<Option<Tracked<'f>>, CatalogError> {
    if !scope.includes(record, filter) {
        return Ok(None);
    }
    let selected = match selection::choose(record, filter) {
        Ok(Some(selected)) => selected,
        Ok(None) => return Ok(None),
        Err(error) => return Ok(Some(candidate(record, &error, filter))),
    };
    if selected.policy == ReadPolicy::SkipBefore {
        let mut ledger = ExportLedger::default();
        ledger.observe(selected.observation(record, SourceDisposition::SkippedBeforeRead));
        return Ok(Some(Tracked {
            piece: Some(Piece::Skipped),
            ledger,
        }));
    }
    let tree = match trees.build(record) {
        Ok(tree) => tree,
        Err(error) => {
            return Ok(Some(failure(
                selected,
                record,
                Fault::Read(&error),
                ExportLedger::default(),
                filter,
            )));
        }
    };
    let mut ledger = crate::export_field_walk::inspect(
        &tree,
        record,
        selected.action.kind(),
        selected.role,
        cancel,
    )?;
    let piece = match projection::project(selected.action, &tree, selected.role, &mut ledger) {
        Ok(piece) => piece,
        Err(error) => {
            return Ok(Some(failure(
                selected,
                record,
                Fault::Projection(&error),
                ledger,
                filter,
            )));
        }
    };
    ledger.observe(selected.observation(record, selected.successful_disposition()));
    Ok(Some(Tracked { piece, ledger }))
}

fn inventories(ledger: &mut ExportLedger) {
    for kind in [
        Kind::Blueprints,
        Kind::GameplayProperties,
        Kind::QualityDistributions,
        Kind::QualityQuantizations,
        Kind::BlueprintPools,
        Kind::DismantleProcesses,
    ] {
        ledger.inventory(kind);
    }
}

pub(crate) fn read<'f>(
    forge: &'f DataForge<'f>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    progress: Option<&(dyn Fn(CatalogProgress) + Sync)>,
) -> Result<ReadCrafting<'f>, CatalogError> {
    let records: Vec<_> = forge.records().collect();
    read_scoped(
        &records,
        filter,
        cancel,
        progress,
        ResourceLinkScope::AllResources,
    )
}

pub(crate) fn read_catalog<'f>(
    forge: &'f DataForge<'f>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    progress: Option<&(dyn Fn(CatalogProgress) + Sync)>,
) -> Result<ReadCrafting<'f>, CatalogError> {
    let records: Vec<_> = forge.records().collect();
    let scope = ResourceLinkScope::catalog(&records, filter, cancel)?;
    read_scoped(&records, filter, cancel, progress, scope)
}

fn read_scoped<'f>(
    records: &[Record<'f>],
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    progress: Option<&(dyn Fn(CatalogProgress) + Sync)>,
    scope: ResourceLinkScope,
) -> Result<ReadCrafting<'f>, CatalogError> {
    let tracked = records
        .par_iter()
        .map_init(TreeBuilder::new, |trees, record| {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            one(trees, record, filter, cancel, scope)
        })
        .filter_map(Result::transpose)
        .collect::<Result<Vec<_>, _>>()?;
    let mut result = super::assemble::assemble(tracked);
    inventories(&mut result.ledger);
    if !result.data.pools.is_empty() {
        let found = crafting_scan::reward_sources(records, &result.data.pools, cancel, progress)?;
        result.data.reward_sources = found.sources;
        result.ledger.merge(found.ledger);
        if found.unreadable > 0 {
            result
                .data
                .warnings
                .push(CraftingWarning::UnreadableRewardRecords(found.unreadable));
        }
    }
    if let Some(warning) = result.data.unknown_ranges_warning() {
        result.data.warnings.push(warning);
    }
    let processes = result.data.live_dismantle().count();
    if processes > 1 {
        result
            .data
            .warnings
            .push(CraftingWarning::AmbiguousDismantle(processes));
    }
    Ok(result)
}
