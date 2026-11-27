// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_comparison::{Comparison, EntryCounts};

/// The counts of the changes below one top-level directory.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DirectorySummary {
    /// The top-level directory as the first change below it spells it; empty for the root.
    pub directory: String,
    /// The counts below it.
    pub counts: EntryCounts,
}

/// The counts of a Comparison per change kind and per top-level directory.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Summary {
    /// The counts per change kind.
    pub counts: EntryCounts,
    /// Matched Entry pairs whose declared checksum algorithms differ.
    pub incomparables: usize,
    /// The counts per top-level directory, ordered by directory ignoring case.
    pub directories: Vec<DirectorySummary>,
}

impl Summary {
    /// Takes the counts of `comparison`.
    pub(crate) fn of(comparison: &Comparison<'_>) -> Self {
        Self {
            counts: comparison.entry_counts(),
            incomparables: comparison.incomparable_entries().len(),
            directories: comparison
                .directory_counts()
                .into_iter()
                .map(|directory| DirectorySummary {
                    directory: directory.directory.to_owned(),
                    counts: directory.counts,
                })
                .collect(),
        }
    }
}
