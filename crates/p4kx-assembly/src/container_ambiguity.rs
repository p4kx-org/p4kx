// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Entry;
use thiserror::Error;

/// A container selector matches several physical Entries in its owning Archive.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("container {name:?} is ambiguous in Archive layer {depth} (Entry indexes {entries:?})")]
pub struct ContainerAmbiguity {
    name: String,
    depth: usize,
    entries: Vec<usize>,
}

impl ContainerAmbiguity {
    /// Builds a diagnostic for the matches at `depth`, with zero for the outer Archive.
    #[must_use]
    pub fn new(name: &str, depth: usize, entries: &[&Entry]) -> Self {
        Self {
            name: name.to_owned(),
            depth,
            entries: entries.iter().map(|entry| entry.index()).collect(),
        }
    }
}
