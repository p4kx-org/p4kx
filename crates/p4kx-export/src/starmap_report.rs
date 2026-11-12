// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::Guid;

use crate::starmap_containers::proof::PlacementProof;

/// Where one Starmap Record is placed: the first Child element that names it.
#[derive(Debug)]
pub(crate) struct Placement {
    pub(crate) system: Arc<str>,
    pub(crate) position: [f64; 3],
    pub(crate) entity_name: String,
    pub(crate) placed_under: Option<Guid>,
    pub(crate) proof: PlacementProof,
}

/// What reading the system containers found, besides the placements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ContainerCounts {
    pub(crate) systems: usize,
    pub(crate) unreadable: usize,
    pub(crate) missing: usize,
    pub(crate) malformed: usize,
    pub(crate) ambiguous: usize,
}

/// What reading the system containers saw, for a person to check the placements by.
///
/// The lines are diagnostics: their wording is not a contract and not part of the catalog.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StarmapReport {
    pub(crate) containers: Vec<String>,
    pub(crate) issues: Vec<String>,
    pub(crate) repeated: Vec<String>,
    pub(crate) unreferenced: Vec<String>,
    pub(crate) deepest_external: usize,
    pub(crate) elapsed: std::time::Duration,
}

impl StarmapReport {
    /// One line per container read that names starmap records, up to 300 per star system, then
    /// a line with the total: its path, the Child elements below it (nested external containers
    /// included), those naming a starmap record and those naming an external container.
    #[must_use]
    pub fn containers(&self) -> &[String] {
        &self.containers
    }

    /// One line per container, placement or limit that left objects without a position, with
    /// the reason, up to 300 per star system; also the containers read from a fallback XML and
    /// the elements with a second child list. A missing or unreadable container is counted once
    /// per Child that names it, not once per distinct container.
    #[must_use]
    pub fn issues(&self) -> &[String] {
        &self.issues
    }

    /// One line per record that more than one Child element places, up to 100.
    #[must_use]
    pub fn repeated(&self) -> &[String] {
        &self.repeated
    }

    /// The socpak containers below ObjectContainers/PU/system that no Child names, up to 200,
    /// the last line giving the total.
    #[must_use]
    pub fn unreferenced(&self) -> &[String] {
        &self.unreferenced
    }

    /// The longest chain of external containers the walk followed, counted from the system
    /// container (0); the limit is 32.
    #[must_use]
    pub const fn deepest_external(&self) -> usize {
        self.deepest_external
    }

    /// How long reading the system containers took.
    #[must_use]
    pub const fn elapsed(&self) -> std::time::Duration {
        self.elapsed
    }
}

/// Placements of one record closer than this, in metres, are one place.
const SAME_PLACE: f64 = 1.0;

/// The longest quoted attribute value in a report line.
const QUOTE_LEN: usize = 64;

/// Returns `value` quoted for a report line, cut to 64 characters.
pub(crate) fn clip(value: &str) -> String {
    let mut quoted: String = value.chars().take(QUOTE_LEN).collect();
    if value.chars().count() > QUOTE_LEN {
        quoted.push('…');
    }
    format!("{quoted:?}")
}

pub(crate) fn apart(a: [f64; 3], b: [f64; 3]) -> bool {
    a.iter()
        .zip(b)
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<f64>()
        .sqrt()
        > SAME_PLACE
}
