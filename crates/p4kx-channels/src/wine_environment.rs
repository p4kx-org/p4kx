// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsString;
use std::path::PathBuf;

use crate::discovery_limits::DiscoveryLimits;
use crate::discovery_report::DiscoveryReport;
use crate::prefix_candidates::{self, PrefixSeeds};
use crate::prefix_discovery;
use crate::prefix_fs::{NativePrefixFileSystem, PrefixFileSystem};

/// Owned environment and configured roots used by Wine-prefix discovery.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WineEnvironment {
    pub(crate) home: Option<PathBuf>,
    pub(crate) wine_prefix: Option<OsString>,
    pub(crate) compat_data: Option<OsString>,
    pub(crate) data_home: Option<PathBuf>,
    pub(crate) steam_roots: Vec<PathBuf>,
    pub(crate) steam_libraries: Vec<PathBuf>,
    pub(crate) bottles_roots: Vec<PathBuf>,
}

impl WineEnvironment {
    /// Creates an environment from the home folder and the value of WINEPREFIX.
    #[must_use]
    pub const fn new(home: Option<PathBuf>, wine_prefix: Option<OsString>) -> Self {
        Self {
            home,
            wine_prefix,
            compat_data: None,
            data_home: None,
            steam_roots: Vec::new(),
            steam_libraries: Vec::new(),
            bottles_roots: Vec::new(),
        }
    }

    /// Captures the current process environment without filesystem discovery.
    #[must_use]
    pub fn from_process() -> Self {
        Self::new(dirs::home_dir(), std::env::var_os("WINEPREFIX"))
            .with_compat_data(std::env::var_os("STEAM_COMPAT_DATA_PATH"))
            .with_data_home(std::env::var_os("XDG_DATA_HOME").map(PathBuf::from))
    }

    /// Sets the explicit Steam compatibility-data directory, above its pfx directory.
    #[must_use]
    pub fn with_compat_data(mut self, value: Option<OsString>) -> Self {
        self.compat_data = value;
        self
    }

    /// Sets the XDG data root; a relative value falls back to the conventional home root.
    #[must_use]
    pub fn with_data_home(mut self, value: Option<PathBuf>) -> Self {
        self.data_home = value;
        self
    }

    /// Sets explicitly configured Steam installation roots in their supplied order.
    #[must_use]
    pub fn with_steam_roots(mut self, paths: Vec<PathBuf>) -> Self {
        self.steam_roots = paths;
        self
    }

    /// Sets explicitly configured Steam library roots in their supplied order.
    #[must_use]
    pub fn with_steam_libraries(mut self, paths: Vec<PathBuf>) -> Self {
        self.steam_libraries = paths;
        self
    }

    /// Sets Bottles storage roots containing immediate bottle directories.
    #[must_use]
    pub fn with_bottles_roots(mut self, paths: Vec<PathBuf>) -> Self {
        self.bottles_roots = paths;
        self
    }

    /// Returns pure, ordered discovery seeds without reading a directory or configuration.
    #[must_use]
    pub fn seeds(&self) -> PrefixSeeds {
        prefix_candidates::seeds(self)
    }

    /// Returns direct candidates without checking their existence.
    #[must_use]
    pub fn candidate_prefixes(&self) -> Vec<PathBuf> {
        self.seeds().direct
    }

    /// Discovers existing prefixes with the production filesystem and default bounds.
    #[must_use]
    pub fn discover(&self) -> DiscoveryReport<Vec<PathBuf>> {
        self.discover_with(&NativePrefixFileSystem, DiscoveryLimits::default())
    }

    /// Discovers prefixes through an injected filesystem under aggregate bounds.
    #[must_use]
    pub fn discover_with<F: PrefixFileSystem>(
        &self,
        fs: &F,
        limits: DiscoveryLimits,
    ) -> DiscoveryReport<Vec<PathBuf>> {
        prefix_discovery::discover(self, fs, limits)
    }

    /// Returns usable discovered prefixes; discover retains failure diagnostics.
    #[must_use]
    pub fn existing_prefixes(&self) -> Vec<PathBuf> {
        self.discover().into_value()
    }
}
