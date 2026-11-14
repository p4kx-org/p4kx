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
