// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod job;
mod name;
mod output;
mod refusal;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

use p4kx_export::{CatalogPhase, CatalogProgress, CatalogPublicationMode, CatalogSummary, Filter};

pub use job::{CatalogExport, CatalogExportStatus};
pub(crate) use job::{refused, refused_gzip, start};
pub(crate) use name::{is_gzip, normalised_catalog_file};

use crate::read_gate::ReadGate;

/// The file name the save dialog of the Basetool catalog suggests: the compressed form the
/// Basetool takes for upload; plain `.json` stays selectable.
pub const CATALOG_FILE: &str = "basetool-catalog.json.zst";

/// The distinct filename proposed for explicitly requested diagnostic catalogs.
pub const DIAGNOSTIC_CATALOG_FILE: &str = "basetool-catalog.diagnostic.json.zst";

/// The selection and publication policy of one catalog job.
#[derive(Debug, Default)]
pub struct CatalogExportOptions {
    /// The source-Record Filter, or the full source selection.
    pub filter: Option<Filter>,
    /// Whether this one job may publish an explicitly marked diagnostic result.
    pub publication_mode: CatalogPublicationMode,
}

/// The step a running Basetool catalog is at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogExportStage {
    /// DataForge is read from the Archive.
    Reading,
    /// The Records are checked and the catalog is built.
    Building,
    /// The catalog file is written.
    Writing,
}

/// How far a running Basetool catalog has come.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogExportProgress {
    /// The step it is at.
    pub stage: CatalogExportStage,
    /// Whether the Records are being read for the crafting data, the first part of building.
    pub crafting: bool,
    /// Records of the DataForge checked so far in the current part.
    pub done: usize,
    /// Records of the DataForge, known once building starts.
    pub total: usize,
}

/// The outcome of a Basetool catalog that was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogExportSummary {
    file: PathBuf,
    summary: CatalogSummary,
}

impl CatalogExportSummary {
    /// Returns the catalog file written.
    #[must_use]
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Returns what the catalog holds: the counts per kind, the texts and the size.
    #[must_use]
    pub const fn summary(&self) -> &CatalogSummary {
        &self.summary
    }
}

const PHASE_CRAFTING: u8 = 1;
const PHASE_RECORDS: u8 = 2;

/// The state shared with the build thread; `phase` is 0 until the first report, then the number
/// of the phase reported, which only grows.
#[derive(Debug, Default)]
struct Shared {
    stage: AtomicU8,
    phase: AtomicU8,
    done: AtomicUsize,
    total: AtomicUsize,
    gate: ReadGate,
}

impl Shared {
    fn enter(&self, stage: CatalogExportStage) {
        self.stage.store(stage as u8, Ordering::Relaxed);
    }

    /// Takes the progress of the build: a report of an earlier phase that arrives after a later
    /// one is dropped, and the counts start again at 0 with each phase.
    fn report(&self, progress: CatalogProgress) {
        let phase = match progress.phase {
            CatalogPhase::Crafting => PHASE_CRAFTING,
            _ => PHASE_RECORDS,
        };
        let before = self.phase.fetch_max(phase, Ordering::Relaxed);
        if before > phase {
            return;
        }
        if before < phase {
            self.done.store(0, Ordering::Relaxed);
        }
        self.total.store(progress.records_total, Ordering::Relaxed);
        self.done
            .fetch_max(progress.records_done, Ordering::Relaxed);
    }

    fn progress(&self) -> CatalogExportProgress {
        let stage = match self.stage.load(Ordering::Relaxed) {
            0 => CatalogExportStage::Reading,
            1 => CatalogExportStage::Building,
            _ => CatalogExportStage::Writing,
        };
        CatalogExportProgress {
            stage,
            crafting: self.phase.load(Ordering::Relaxed) == PHASE_CRAFTING,
            done: self.done.load(Ordering::Relaxed),
            total: self.total.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(phase: CatalogPhase, done: usize, total: usize) -> CatalogProgress {
        CatalogProgress {
            phase,
            records_done: done,
            records_total: total,
        }
    }

    #[test]
    fn the_counts_start_again_with_the_records_phase_and_late_crafting_reports_are_dropped() {
        let shared = Shared::default();
        assert!(
            !shared.progress().crafting,
            "nothing is read before the first report"
        );
        shared.report(at(CatalogPhase::Crafting, 900, 1000));
        let crafting = shared.progress();
        assert!(crafting.crafting);
        assert_eq!((crafting.done, crafting.total), (900, 1000));
        shared.report(at(CatalogPhase::Records, 10, 1000));
        let records = shared.progress();
        assert!(!records.crafting);
        assert_eq!((records.done, records.total), (10, 1000));
        shared.report(at(CatalogPhase::Crafting, 1000, 1000));
        let late = shared.progress();
        assert!(!late.crafting, "an earlier phase does not return");
        assert_eq!(late.done, 10);
    }
}
