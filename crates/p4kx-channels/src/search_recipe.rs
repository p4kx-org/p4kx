// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::path::PathBuf;

use p4kx_settings::{Settings, WineLocations};

use crate::detect::SearchPaths;
use crate::discovery_context::DiscoveryContext;
use crate::discovery_limits::{DiscoveryLimit, DiscoveryLimits};
use crate::discovery_report::DiscoveryReport;
use crate::launcher_snapshot::LauncherSnapshot;
use crate::prefix_discovery;
use crate::prefix_fs::{NativePrefixFileSystem, PrefixFileSystem};
use crate::wine::WineEnvironment;

/// Captured search inputs whose filesystem topology is refreshed for every snapshot.
#[derive(Debug, Clone, Default)]
pub struct SearchRecipe {
    base: SearchPaths,
    environment: Option<WineEnvironment>,
    limits: DiscoveryLimits,
}

impl SearchRecipe {
    /// Captures platform defaults and environment without reading prefixes or launcher logs.
    #[must_use]
    pub fn standard() -> Self {
        Self {
            base: SearchPaths::platform_defaults(),
            environment: cfg!(unix).then(WineEnvironment::from_process),
            limits: DiscoveryLimits::default(),
        }
    }

    /// Creates an explicit recipe without any process-environment lookup.
    #[must_use]
    pub fn explicit(paths: SearchPaths) -> Self {
        Self {
            base: paths,
            environment: None,
            limits: DiscoveryLimits::default(),
        }
    }

    /// Creates an owned injected recipe for current filesystem discovery.
    #[must_use]
    pub const fn with_environment(
        base: SearchPaths,
        environment: WineEnvironment,
        limits: DiscoveryLimits,
    ) -> Self {
        Self {
            base,
            environment: Some(environment),
            limits,
        }
    }

    /// Replaces the native default library slot.
    #[must_use]
    pub fn with_default_library(mut self, path: PathBuf) -> Self {
        self.base.default_library = Some(path);
        self
    }

    /// Replaces the native launcher slot without removing prefix sources.
    #[must_use]
    pub fn with_launcher(mut self, path: PathBuf) -> Self {
        self.base.launcher = Some(path);
        self
    }

    /// Replaces configured Wine prefixes and `CrossOver` bottle roots.
    #[must_use]
    pub fn with_wine_locations(
        mut self,
        prefixes: Vec<PathBuf>,
        crossover_bottle_roots: Vec<PathBuf>,
    ) -> Self {
        if self.environment.is_some() || !prefixes.is_empty() || !crossover_bottle_roots.is_empty()
        {
            let environment = self
                .environment
                .get_or_insert_with(WineEnvironment::default);
            environment.wine_prefixes = prefixes;
            environment.crossover_bottle_roots = crossover_bottle_roots;
        }
        self
    }

    /// Replaces manual inputs without adding a second discovery pass.
    #[must_use]
    pub fn with_manual(mut self, folders: Vec<PathBuf>, archives: Vec<PathBuf>) -> Self {
        self.base.manual_folders = folders;
        self.base.manual_archives = archives;
        self
    }

    /// Refreshes configured prefix topology with the production filesystem.
    #[must_use]
    pub fn refresh(&self) -> DiscoveryReport<SearchPaths> {
        self.refresh_with(&NativePrefixFileSystem)
    }

    /// Refreshes prefix topology through an injected filesystem.
    #[must_use]
    pub fn refresh_with<F: PrefixFileSystem>(&self, fs: &F) -> DiscoveryReport<SearchPaths> {
        let mut context = DiscoveryContext::new(fs, self.limits);
        let paths = self.paths(&mut context, None, None);
        DiscoveryReport::new(paths, context.issues)
    }

    /// Opens the current source snapshot through one aggregate discovery budget.
    #[must_use]
    pub fn snapshot(&self) -> LauncherSnapshot {
        self.snapshot_with(&NativePrefixFileSystem)
    }

    /// Opens current sources with borrowed manual inputs under the same discovery budget.
    #[must_use]
    pub fn snapshot_with_manual(
        &self,
        folders: &[PathBuf],
        archives: &[PathBuf],
    ) -> LauncherSnapshot {
        self.snapshot_in(&NativePrefixFileSystem, Some((folders, archives)), None)
    }

    /// Opens current sources with the profile's saved manual and Wine locations.
    #[must_use]
    pub fn snapshot_with_settings(&self, settings: &Settings) -> LauncherSnapshot {
        self.snapshot_in(
            &NativePrefixFileSystem,
            Some((&settings.channels.folders, &settings.channels.archives)),
            Some(&settings.wine),
        )
    }

    /// Opens current sources with saved Wine locations and the recipe's manual inputs.
    #[must_use]
    pub fn snapshot_with_saved_wine(&self, wine: &WineLocations) -> LauncherSnapshot {
        self.snapshot_in(&NativePrefixFileSystem, None, Some(wine))
    }

    /// Opens a source snapshot after injected prefix and source-location discovery.
    #[must_use]
    pub fn snapshot_with<F: PrefixFileSystem>(&self, fs: &F) -> LauncherSnapshot {
        self.snapshot_in(fs, None, None)
    }

    fn snapshot_in<F: PrefixFileSystem>(
        &self,
        fs: &F,
        manual: Option<(&[PathBuf], &[PathBuf])>,
        wine: Option<&WineLocations>,
    ) -> LauncherSnapshot {
        let mut context = DiscoveryContext::new(fs, self.limits);
        let paths = self.paths(&mut context, manual, wine);
        LauncherSnapshot::build(paths, context)
    }

    fn paths<F: PrefixFileSystem>(
        &self,
        context: &mut DiscoveryContext<'_, F>,
        manual: Option<(&[PathBuf], &[PathBuf])>,
        wine: Option<&WineLocations>,
    ) -> SearchPaths {
        let (folders, archives) =
            manual.unwrap_or((&self.base.manual_folders, &self.base.manual_archives));
        let mut paths = bounded_manual_paths(&self.base, context, folders, archives);
        let explicit = WineEnvironment::new(None, None);
        let environment = self.environment.as_ref().or_else(|| {
            wine.filter(|locations| !locations.is_empty())
                .map(|_| &explicit)
        });
        if let Some(environment) = environment {
            for prefix in prefix_discovery::discover_in(environment, wine, context) {
                if !paths.wine_prefixes.contains(&prefix) {
                    paths.wine_prefixes.push(prefix);
                }
            }
        }
        paths
    }
}

pub(crate) fn bounded_paths<F: PrefixFileSystem>(
    base: &SearchPaths,
    context: &mut DiscoveryContext<'_, F>,
) -> SearchPaths {
    bounded_manual_paths(base, context, &base.manual_folders, &base.manual_archives)
}

fn bounded_manual_paths<F: PrefixFileSystem>(
    base: &SearchPaths,
    context: &mut DiscoveryContext<'_, F>,
    folders: &[PathBuf],
    archives: &[PathBuf],
) -> SearchPaths {
    let mut prefixes = HashSet::new();
    let mut paths = SearchPaths {
        default_library: base.default_library.clone(),
        launcher: base.launcher.clone(),
        wine_prefixes: Vec::new(),
        manual_folders: folders.to_vec(),
        manual_archives: archives.to_vec(),
    };
    for prefix in &base.wine_prefixes {
        if !context.admit(prefix, DiscoveryLimit::Roots, 1) {
            break;
        }
        if !prefixes.insert(prefix) {
            continue;
        }
        if !context.admit(prefix, DiscoveryLimit::Prefixes, 1) {
            break;
        }
        paths.wine_prefixes.push(prefix.clone());
    }
    paths
}
