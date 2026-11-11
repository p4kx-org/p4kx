// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use crate::prefix_seed_walk::{self, SeedKind};

use crate::wine::WineEnvironment;

/// Pure candidate roots before any filesystem access.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrefixSeeds {
    pub(crate) direct: Vec<PathBuf>,
    pub(crate) steam_roots: Vec<PathBuf>,
    pub(crate) steam_libraries: Vec<PathBuf>,
    pub(crate) bottles: Vec<PathBuf>,
    pub(crate) crossover: Vec<PathBuf>,
    pub(crate) games: Vec<PathBuf>,
}

impl PrefixSeeds {
    /// Returns explicitly named and conventional direct prefixes in priority order.
    #[must_use]
    pub fn direct_prefixes(&self) -> &[PathBuf] {
        &self.direct
    }

    /// Returns the Steam installation roots whose known configuration is examined.
    #[must_use]
    pub fn steam_roots(&self) -> &[PathBuf] {
        &self.steam_roots
    }

    /// Returns explicit Steam library roots.
    #[must_use]
    pub fn steam_libraries(&self) -> &[PathBuf] {
        &self.steam_libraries
    }

    /// Returns roots whose immediate children are Bottles prefixes.
    #[must_use]
    pub fn bottles_roots(&self) -> &[PathBuf] {
        &self.bottles
    }

    /// Returns roots whose immediate children are `CrossOver` bottles.
    #[must_use]
    pub fn crossover_bottle_roots(&self) -> &[PathBuf] {
        &self.crossover
    }

    /// Returns roots whose immediate children are custom or Lutris prefixes.
    #[must_use]
    pub fn games_roots(&self) -> &[PathBuf] {
        &self.games
    }
}

pub(crate) fn seeds(environment: &WineEnvironment) -> PrefixSeeds {
    let mut seeds = PrefixSeeds::default();
    prefix_seed_walk::walk(environment, None, |kind, path| {
        let paths = match kind {
            SeedKind::Direct => &mut seeds.direct,
            SeedKind::Steam => &mut seeds.steam_roots,
            SeedKind::Library => &mut seeds.steam_libraries,
            SeedKind::Bottles => &mut seeds.bottles,
            SeedKind::CrossOver => &mut seeds.crossover,
            SeedKind::Games => &mut seeds.games,
        };
        if !paths.iter().any(|known| known == path) {
            paths.push(path.to_path_buf());
        }
        true
    });
    seeds
}
