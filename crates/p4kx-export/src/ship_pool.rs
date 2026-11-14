// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::{Arc, Mutex, PoisonError};

use p4kx_assembly::{ArchiveFiles, Entities};
use p4kx_extraction::{Record, RecordTree};

use crate::ship_base::{ShipRead, ShipReader};
use crate::{CatalogFailure, CatalogReadFailure};

#[cfg(test)]
mod tests;

/// The ship readers not in use. A reader keeps the shared Entities and containers it has read, so
/// a reader taken for one ship and given back for the next keeps its caches across the splits of
/// a parallel scan.
pub(crate) struct ShipPool<'f> {
    entities: Arc<Entities<'f>>,
    files: &'f ArchiveFiles<'f>,
    idle: Mutex<Vec<ShipReader<'f>>>,
    observed: Mutex<Vec<ObservedReader<'f>>>,
}

struct ObservedReader<'f> {
    reader: ShipReader<'f>,
    capture: Arc<crate::loadout::ledger::Capture<'f>>,
}

pub(crate) struct CatalogRead<'f> {
    pub(crate) read: Result<ShipRead, CatalogReadFailure<'f>>,
    pub(crate) ledger: crate::export_ledger::ExportLedger,
}

impl<'f> ShipPool<'f> {
    pub(crate) const fn new(entities: Arc<Entities<'f>>, files: &'f ArchiveFiles<'f>) -> Self {
        Self {
            entities,
            files,
            idle: Mutex::new(Vec::new()),
            observed: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn read(
        &self,
        record: &Record<'f>,
        tree: &RecordTree<'f>,
    ) -> Result<ShipRead, CatalogFailure> {
        let taken = self
            .idle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop();
        let mut reader = taken
            .unwrap_or_else(|| ShipReader::with_entities(Arc::clone(&self.entities), self.files));
        let result = reader
            .read_with_digest(record, tree)
            .map_err(CatalogReadFailure::into_interpreted);
        self.idle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(reader);
        result
    }

    /// Captures actual resolver and cargo dependencies while reusing their physical cache proofs.
    pub(crate) fn read_catalog(
        &self,
        record: &Record<'f>,
        tree: &RecordTree<'f>,
        sources: &crate::export_ledger::ExportLedger,
        input: &'f crate::build::original_input::OriginalCatalogInput<'f, 'f>,
        cancel: Option<&p4kx_extraction::CancelToken>,
    ) -> Result<CatalogRead<'f>, crate::CatalogError> {
        let taken = crate::export_evidence_lock::lock(&self.observed)?.pop();
        let mut observed = taken.unwrap_or_else(|| {
            let capture = Arc::new(crate::loadout::ledger::Capture::for_input(
                input,
                sources,
                cancel,
                crate::Kind::Ships,
            ));
            ObservedReader {
                reader: ShipReader::with_observed_entities(
                    Arc::clone(&self.entities),
                    self.files,
                    Arc::clone(&capture),
                ),
                capture,
            }
        });
        observed.capture.check_input(input)?;
        observed.capture.seed_proofs(sources)?;
        let read = observed.reader.read_with_digest(record, tree);
        let ledger = observed.capture.finish()?;
        crate::export_evidence_lock::lock(&self.observed)?.push(observed);
        Ok(CatalogRead { read, ledger })
    }
}

#[cfg(test)]
#[path = "ship_pool_cache_tests.rs"]
mod cache_tests;
