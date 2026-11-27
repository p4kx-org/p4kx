// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::PublicPatches;
use crate::detect::{self, DetectedChannels, SearchPaths};
use crate::discovery_context::DiscoveryContext;
use crate::discovery_limits::{DiscoveryBudget, DiscoveryLimits};
use crate::discovery_report::{DiscoveryIssue, DiscoveryReport};
use crate::launcher_log::{LAUNCHER_LOG_FILES, Wanted};
use crate::launcher_read;
use crate::launcher_sources;
use crate::launcher_stamp::LauncherStamp;
use crate::opened_log::OpenedLog;
use crate::ordered_libraries::OrderedLibraries;
use crate::prefix_fs::PrefixFileSystem;
use crate::wine_context::WineContext;

#[derive(Debug)]
pub(crate) struct LauncherOccurrence {
    pub(crate) folder: PathBuf,
    pub(crate) prefix: Option<Arc<WineContext>>,
    pub(crate) files: [Option<usize>; LAUNCHER_LOG_FILES.len()],
}

/// A refreshed source set retaining the exact opened regular logs used by its scan.
pub struct LauncherSnapshot {
    pub(crate) paths: SearchPaths,
    pub(crate) stamp: LauncherStamp,
    pub(crate) occurrences: Vec<LauncherOccurrence>,
    pub(crate) logs: Vec<OpenedLog>,
    pub(crate) budget: DiscoveryBudget,
    pub(crate) issues: Vec<DiscoveryIssue>,
}

impl LauncherSnapshot {
    pub(crate) fn build<F: PrefixFileSystem>(
        paths: SearchPaths,
        context: DiscoveryContext<'_, F>,
    ) -> Self {
        launcher_sources::build(paths, context)
    }

    pub(crate) fn log_folder(folder: &Path, limits: DiscoveryLimits) -> Self {
        launcher_sources::log_folder(folder, limits)
    }

    /// Returns the explicit resolved search paths of this snapshot.
    #[must_use]
    pub const fn paths(&self) -> &SearchPaths {
        &self.paths
    }

    /// Returns the source-aware stamp for complete-scan cache admission.
    #[must_use]
    pub const fn stamp(&self) -> &LauncherStamp {
        &self.stamp
    }

    /// Returns discovery and source-open failures.
    #[must_use]
    pub fn issues(&self) -> &[DiscoveryIssue] {
        &self.issues
    }

    /// Returns whether source discovery and opening completed without failures.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.issues.is_empty()
    }

    /// Reads public patches from the exact opened files in source-occurrence order.
    #[must_use]
    pub fn read_patches(&self) -> DiscoveryReport<PublicPatches> {
        let (log, issues) = launcher_read::scan(self, Wanted::PATCHES).into_parts();
        DiscoveryReport::new(log.patches, issues)
    }

    /// Reads default and logged libraries with retained partial-source diagnostics.
    #[must_use]
    pub fn read_libraries(&self) -> DiscoveryReport<Vec<PathBuf>> {
        let (log, issues) = launcher_read::scan(self, Wanted::LIBRARIES).into_parts();
        let mut libraries = OrderedLibraries::default();
        for item in &self.stamp.libraries {
            libraries.insert(item.path.clone());
        }
        for path in log.libraries {
            libraries.insert(path);
        }
        DiscoveryReport::new(libraries.finish(), issues)
    }

    /// Detects Channels and their patches through one scan of the opened files.
    #[must_use]
    pub fn detect(&self) -> DiscoveryReport<DetectedChannels> {
        let (log, issues) = launcher_read::scan(self, Wanted::BOTH).into_parts();
        let libraries = self.stamp.libraries.iter().map(|item| item.path.as_path());
        DiscoveryReport::new(detect::from_log(&self.paths, libraries, log), issues)
    }
}
