// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// Optional work allowances for distinct physical source and placement walks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct StarmapTraversalBudget {
    /// Maximum Child declarations across uniquely parsed physical container descriptors.
    pub source_children: Option<usize>,
    /// Maximum visited Child occurrences per system root, including repeated containers.
    pub placed_children_per_system: Option<usize>,
}

impl StarmapTraversalBudget {
    /// Reads every Child declaration and placement unless the caller selects an allowance.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            source_children: None,
            placed_children_per_system: None,
        }
    }

    /// Sets a shared allowance for physical Child declarations, including zero.
    #[must_use]
    pub const fn with_source_children(mut self, maximum: usize) -> Self {
        self.source_children = Some(maximum);
        self
    }

    /// Sets an independent Child-placement allowance for each system, including zero.
    #[must_use]
    pub const fn with_placed_children_per_system(mut self, maximum: usize) -> Self {
        self.placed_children_per_system = Some(maximum);
        self
    }
}
