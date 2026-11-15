// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::time::SystemTime;

use p4kx_channels::PublicPatches;
use p4kx_extraction::{CancelToken, Filter};

/// The step of a catalog build that a [`CatalogProgress`] reports, in the order the steps run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum CatalogPhase {
    /// The crafting data is read, the reward sources of the blueprint pools among it.
    Crafting,
    /// Every Record is checked for the kinds of the catalog.
    #[default]
    Records,
}

/// How far the build of a catalog has come.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct CatalogProgress {
    /// The step the counts belong to; they start again at 0 with each step.
    pub phase: CatalogPhase,
    /// Records of the DataForge checked so far in this step.
    pub records_done: usize,
    /// Records of the DataForge.
    pub records_total: usize,
}

/// The settings of one catalog build.
#[derive(Clone, Copy)]
pub struct CatalogOptions<'a> {
    /// Requires complete reads by default; diagnostic partial publication must be explicit.
    pub publication_mode: crate::CatalogPublicationMode,
    /// Selects owning catalog Records by path; the global tag taxonomy remains complete.
    pub filter: Option<&'a Filter>,
    /// The export time the catalog states.
    pub exported_at: SystemTime,
    /// Stops the build once cancelled.
    pub cancel: Option<&'a CancelToken>,
    /// Called from any worker thread while Records are checked; calls may overtake each other.
    pub progress: Option<&'a (dyn Fn(CatalogProgress) + Sync)>,
    /// The public patches the catalog's source states for the Build; `None` states none.
    pub public_patches: Option<&'a PublicPatches>,
}

impl CatalogOptions<'_> {
    /// Returns options for a complete catalog exported at `exported_at`, without cancellation,
    /// progress or public patches.
    #[must_use]
    pub const fn new(exported_at: SystemTime) -> Self {
        Self {
            publication_mode: crate::CatalogPublicationMode::Normal,
            filter: None,
            exported_at,
            cancel: None,
            progress: None,
            public_patches: None,
        }
    }
}

impl fmt::Debug for CatalogOptions<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CatalogOptions")
            .field("publication_mode", &self.publication_mode)
            .field("filter", &self.filter)
            .field("exported_at", &self.exported_at)
            .field("cancel", &self.cancel)
            .field("progress", &self.progress.is_some())
            .field("public_patches", &self.public_patches)
            .finish()
    }
}

/// A stage boundary in the catalog file writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CatalogStage {
    /// Reads the original DataForge Entry from the Archive.
    ReadDataForge,
    /// Opens the DataForge and reads the localization inputs.
    ReadInputs,
    /// Reads crafting Records and Blueprint reward sources.
    Crafting,
    /// Locates and walks the system containers.
    WorldPlacements,
    /// Inspects the Records and prepares selected rows.
    Records,
    /// Resolves contextual and derived data.
    Projections,
    /// Compiles the publication health and checks the publication policy.
    ExportHealth,
    /// Freezes the source and contribution inventory.
    Provenance,
    /// Measures the JSON size for the Zstd frame header.
    MeasureJson,
    /// Writes the JSON to the chosen output sink.
    WriteJson,
}

impl CatalogStage {
    /// Returns the stable identifier printed in catalog phase logs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadDataForge => "read-dataforge",
            Self::ReadInputs => "read-inputs",
            Self::Crafting => "crafting",
            Self::WorldPlacements => "world-placements",
            Self::Records => "records",
            Self::Projections => "projections",
            Self::ExportHealth => "export-health",
            Self::Provenance => "provenance",
            Self::MeasureJson => "measure-json",
            Self::WriteJson => "write-json",
        }
    }
}

/// Options and observation for one preparation and write.
#[derive(Clone, Copy)]
pub(crate) struct CatalogRun<'run, 'inputs> {
    pub(crate) options: &'run CatalogOptions<'inputs>,
    report: Option<&'run dyn Fn(CatalogStage)>,
}

impl<'run, 'inputs> CatalogRun<'run, 'inputs> {
    pub(crate) const fn new(
        options: &'run CatalogOptions<'inputs>,
        report: Option<&'run dyn Fn(CatalogStage)>,
    ) -> Self {
        Self { options, report }
    }

    pub(crate) fn begin(self, stage: CatalogStage) {
        if let Some(report) = self.report {
            report(stage);
        }
    }
}
