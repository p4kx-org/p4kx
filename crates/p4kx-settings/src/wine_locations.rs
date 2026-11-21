// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Saved Wine and `CrossOver` source locations.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(default)]
pub struct WineLocations {
    /// Absolute paths to individual Wine prefixes.
    pub prefixes: Vec<PathBuf>,
    /// Absolute roots whose immediate children are `CrossOver` bottles.
    pub crossover_bottle_roots: Vec<PathBuf>,
}

impl WineLocations {
    /// Creates the saved locations in their discovery order.
    #[must_use]
    pub const fn new(prefixes: Vec<PathBuf>, crossover_bottle_roots: Vec<PathBuf>) -> Self {
        Self {
            prefixes,
            crossover_bottle_roots,
        }
    }

    /// Returns whether no configured locations were supplied.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.prefixes.is_empty() && self.crossover_bottle_roots.is_empty()
    }
}
