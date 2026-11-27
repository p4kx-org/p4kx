// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_assembly::{ArchiveFiles, Entities};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Record, RecordTree, SourceInventory,
    TreeBuilder,
};
use rayon::prelude::*;

use super::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::authored_fields::ship_source_read::standalone;
use crate::export_diagnostic::ExportSourceRole;
use crate::playability_rules::{Evidence, ShipDependencies};
use crate::record_observation::RecordObservation;
use crate::ship_base::{ShipBase, ShipBaseError, ShipBaseEvidence, ShipBaseOptions};
use crate::ship_pool::ShipPool;
use crate::{
    CatalogError, CatalogFailure, CatalogPhase, CatalogProgress, ExportAccountingError, Kind,
};

#[cfg(test)]
#[path = "build_standalone_ships_tests.rs"]
mod tests;

pub(crate) struct ObservedShips {
    pub(crate) rows: Vec<(ShipBase, ShipBaseEvidence)>,
    pub(crate) failures: Vec<CatalogFailure>,
    pub(crate) provenance: SourceInventory,
}

pub(crate) fn read<'source>(
    archive: &'source Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'source>,
    records: &[Record<'source>],
    options: &ShipBaseOptions<'_>,
) -> Result<ObservedShips, ShipBaseError> {
    let watch = ArchiveWatch::new(archive);
    watch.check().map_err(CatalogError::from)?;
    let origins = CatalogOrigins::for_archive_records(archive, entry)?;
    let input = OriginalCatalogInput::new(forge, &origins);

    let files = ArchiveFiles::new(archive);
    let pool = ShipPool::new(Arc::new(Entities::new(forge)), &files);
    let reader = Reader {
        input: &input,
        pool: &pool,

        options,
    };
    let (rows, failures) = reader.read(records)?;
    let provenance = origins.uses.finish_with(|| checkpoint(options.cancel))?;
    watch.check().map_err(CatalogError::from)?;
    Ok(ObservedShips {
        rows,
        failures,
        provenance,
    })
}

fn prepare_trees<'source>(
    records: &[Record<'source>],
    options: &ShipBaseOptions<'_>,
) -> Result<Vec<Result<RecordTree<'source>, CatalogFailure>>, CatalogError> {
    records
        .par_iter()
        .map_init(TreeBuilder::new, |trees, record| {
            checkpoint(options.cancel)?;
            Ok(trees
                .build(record)
                .map_err(|error| CatalogFailure::of(record, &error)))
        })
        .collect()
}

type Row = (ShipBase, ShipBaseEvidence);

struct Reader<'read, 'source> {
    input: &'source OriginalCatalogInput<'source, 'source>,
    pool: &'read ShipPool<'source>,

    options: &'read ShipBaseOptions<'read>,
}

impl<'source> Reader<'_, 'source> {
    fn read(
        &self,
        records: &[Record<'source>],
    ) -> Result<(Vec<Row>, Vec<CatalogFailure>), ShipBaseError> {
        let mut dependencies = ShipDependencies::new(self.input);
        let done = AtomicUsize::new(0);
        let mut rows = Vec::new();
        let mut failures = Vec::new();
        for batch in records.chunks(rayon::current_num_threads()) {
            let trees = prepare_trees(batch, self.options)?;
            dependencies.prepare(
                trees.iter().filter_map(|tree| tree.as_ref().ok()),
                self.options.cancel,
            )?;
            let evidence = dependencies.evidence();
            let reads: Vec<_> = trees
                .into_par_iter()
                .zip(batch.par_iter())
                .map(|(tree, record)| {
                    checkpoint(self.options.cancel)?;
                    let result = match tree {
                        Ok(tree) => self.read_owner(record, &tree, evidence),
                        Err(failure) => Ok(Err(failure)),
                    };
                    let finished = done.fetch_add(1, Ordering::Relaxed).saturating_add(1);
                    if let Some(progress) = self.options.progress {
                        progress(CatalogProgress {
                            phase: CatalogPhase::Records,
                            records_done: finished,
                            records_total: records.len(),
                        });
                    }
                    result
                })
                .collect();
            for read in reads {
                match read? {
                    Ok(row) => rows.push(row),
                    Err(failure) => failures.push(failure),
                }
            }
        }
        Ok((rows, failures))
    }

    fn read_owner(
        &self,
        record: &Record<'source>,
        tree: &RecordTree<'source>,
        evidence: &Evidence,
    ) -> Result<Result<Row, CatalogFailure>, ShipBaseError> {
        let admission = RecordObservation::capture(self.input, record)?;
        let inputs = standalone::read(tree, self.input, &admission, self.options.cancel)?;
        let ledger = crate::export_field_walk::inspect_observed(
            Some(self.input),
            tree,
            &admission,
            Some(Kind::Ships),
            ExportSourceRole::Selected,
            self.options.cancel,
        )?;
        let observed = self.pool.read_catalog(
            &inputs.operands,
            record,
            tree,
            &ledger,
            self.input,
            self.options.cancel,
        )?;
        let base = match observed.read {
            Ok(read) => read.base,
            Err(failure) => return Ok(Err(failure.into_interpreted())),
        };
        let aggregates = observed
            .aggregates
            .ok_or_else(|| CatalogError::from(ExportAccountingError::UnobservedFailure))?;
        let playability = inputs
            .playability
            .decide(tree, inputs.vehicle, evidence, self.input)?;
        let exosuit_kind = inputs.playability.exosuit_kind();
        let kind = exosuit_kind
            .as_ref()
            .unwrap_or(&aggregates.vehicle.vehicle_kind);
        let evidence =
            ShipBaseEvidence::collect(&base, &inputs.header, kind, playability, &aggregates)?;
        Ok(Ok((base, evidence)))
    }
}
