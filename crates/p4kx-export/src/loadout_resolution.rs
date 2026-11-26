// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{ArchiveFiles, Entities, LoadoutRecordReader, LoadoutResolver};
use p4kx_extraction::{CancelToken, DataForge, SourceUses};

use super::alignment::AlignmentLookup;
use super::ledger::Capture;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_rows::spool::checkpoint;
use crate::catalog_v3::catalog_item_alignment::ItemAlignmentRows;
use crate::contract_loadout::{EntityKind, Loadout};
use crate::export_ledger::ExportLedger;
use crate::guid_text::{self, CatalogGuid};
use crate::loadout_port::PortReader;
use crate::{CatalogError, CatalogReadFailure, Kind};

/// Retains the read failures and origin ledger for one resolved Loadout scan.
pub(crate) struct ResolvedRows<'source> {
    pub(crate) failures: Vec<CatalogReadFailure<'source>>,
    pub(crate) ledger: ExportLedger,
}

/// Resolves the selected original Loadouts and delivers their typed rows in Catalog order.
/// Publish accepted rows only after this function and the enclosing Archive checks succeed.
pub(crate) fn read_rows<'source>(
    files: &ArchiveFiles<'_>,
    entities: &Arc<Entities<'source>>,
    wanted: impl Iterator<Item = Wanted>,
    sources: ResolverEvidence<'_>,
    input: &OriginalCatalogInput<'_, 'source>,
    cancel: Option<&CancelToken>,
    accept: impl FnMut(Loadout) -> Result<(), CatalogError>,
) -> Result<ResolvedRows<'source>, CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let ResolverEvidence {
        seed,
        alignment,
        record_reader,
        use_compaction,
    } = sources;
    let capture = Arc::new(Capture::for_input(input, seed, cancel, Kind::Loadouts));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::clone(entities), capture.clone())
            .with_files(files)
            .stopping_at_docked_vehicles();
    if let Some(reader) = record_reader {
        resolver = resolver.with_record_cache(reader);
    }
    let mut reader = PortReader::with_alignment(alignment, files.archive());
    let failures = resolve_ordered(
        wanted,
        cancel,
        |want| read_with(&mut resolver, &mut reader, entities.forge(), want, &capture),
        accept,
        |completed| use_compaction.after_owner(completed, &input.origins().uses, cancel),
    );
    drop(resolver);
    let mut ledger = capture.finish()?;
    drop(capture);
    let failures = failures?;
    ledger.inventory(Kind::Loadouts);
    Ok(ResolvedRows { failures, ledger })
}

/// An entry of `ships` or `items` whose ports are wanted.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Wanted {
    pub(crate) record_index: u32,
    pub(crate) guid: CatalogGuid,
    pub(crate) kind: EntityKind,
}

#[derive(Clone, Copy)]
pub(crate) struct SelectedSources<'rows, 'source> {
    pub(crate) ledger: &'rows ExportLedger,
    pub(crate) alignment: &'rows ItemAlignmentRows<'source>,
}

/// Shared origin seed and physical configuration reader for a resolved Loadout scan.
pub(crate) struct ResolverEvidence<'scope> {
    pub(crate) seed: &'scope ExportLedger,
    pub(crate) alignment: &'scope mut dyn AlignmentLookup,
    pub(crate) record_reader: Option<&'scope dyn LoadoutRecordReader>,
    pub(crate) use_compaction: UseCompaction,
}

#[derive(Clone, Copy)]
pub(crate) enum UseCompaction {
    AtProjectionEnd,
    BetweenCatalogBatches,
}

impl UseCompaction {
    pub(super) fn after_owner(
        self,
        completed: usize,
        uses: &SourceUses,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        const OWNERS_PER_BATCH: usize = 64;
        if matches!(self, Self::BetweenCatalogBatches) && completed.is_multiple_of(OWNERS_PER_BATCH)
        {
            uses.compact_record_use_batch(|| crate::catalog_query_session::checkpoint(cancel))?;
        }
        Ok(())
    }
}

pub(super) fn resolve_ordered<'source>(
    wanted: impl Iterator<Item = Wanted>,
    cancel: Option<&CancelToken>,
    mut read: impl FnMut(Wanted) -> Result<Option<Loadout>, CatalogReadFailure<'source>>,
    mut accept: impl FnMut(Loadout) -> Result<(), CatalogError>,
    mut after_owner: impl FnMut(usize) -> Result<(), CatalogError>,
) -> Result<Vec<CatalogReadFailure<'source>>, CatalogError> {
    let mut failures = Vec::new();
    for (index, (position, want)) in in_catalog_order(wanted).into_iter().enumerate() {
        checkpoint(cancel)?;
        match read(want) {
            Ok(Some(entry)) => accept(entry)?,
            Ok(None) => {}
            Err(failure) => failures.push((position, failure)),
        }
        after_owner(index + 1)?;
    }
    // Equal-GUID failures are deduplicated in input order by the collector.
    failures.sort_unstable_by_key(|(position, _)| *position);
    Ok(failures.into_iter().map(|(_, failure)| failure).collect())
}

pub(crate) fn in_catalog_order(wanted: impl Iterator<Item = Wanted>) -> Vec<(usize, Wanted)> {
    let mut ordered: Vec<_> = wanted.enumerate().collect();
    guid_text::sort_by_guid(&mut ordered, |(_, want)| want.guid);
    ordered
}

pub(super) fn read_with<'f: 'worker, 'worker>(
    resolver: &mut LoadoutResolver<'worker>,
    reader: &mut PortReader<'_>,
    forge: &'f DataForge<'f>,
    want: Wanted,
    capture: &Capture<'_>,
) -> Result<Option<Loadout>, CatalogReadFailure<'f>> {
    let Some(record) = forge
        .record(want.record_index)
        .filter(|record| record.guid() == want.guid.0)
    else {
        capture.missing_root();
        return Ok(None);
    };
    if let Err(error) = capture.begin_loadout_vehicles() {
        capture.record_error(error);
        return Ok(None);
    }
    let node = match resolver.resolve(&record) {
        Ok(node) => node,
        Err(error) => {
            capture.root(&record, want.kind, Err(&error));
            return Err(CatalogReadFailure::of_loadout(&record, &error));
        }
    };
    capture.root(&record, want.kind, Ok(&node));
    let ports = match reader.ports(&node, capture) {
        Ok(ports) => ports,
        Err(error) => {
            capture.record_error(error);
            return Ok(None);
        }
    };
    let issues = reader.take_issues();
    if want.kind == EntityKind::Item && ports.is_empty() {
        return Ok(None);
    }
    let root = match capture.root_values(&record, want.kind) {
        Ok(root) => root,
        Err(error) => {
            capture.record_error(error);
            return Ok(None);
        }
    };
    Ok(Some(Loadout {
        root_record_index: want.record_index,
        source_record_index: root.index,
        port_inventory_complete: node.issues().is_empty(),
        entity_guid: root.guid,
        entity_kind: root.kind,
        issues,
        ports,
    }))
}
