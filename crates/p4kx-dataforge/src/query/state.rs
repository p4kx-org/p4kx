// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::ValueError;

/// The array budget and unresolved links shared by the columns of one Record row.
#[derive(Debug)]
pub(crate) struct WalkState {
    budget: usize,
    limit: usize,
    unresolved: Vec<usize>,
}

impl WalkState {
    /// Returns the state of a Record no path has read yet.
    pub(crate) const fn new(limit: usize) -> Self {
        Self {
            budget: limit,
            limit,
            unresolved: Vec::new(),
        }
    }

    /// Readies the state for the next Record, keeping its buffer.
    pub(crate) fn reset(&mut self) {
        self.budget = self.limit;
        self.unresolved.clear();
    }

    /// Returns how many array elements this row admitted.
    pub(crate) const fn visited(&self) -> usize {
        self.limit - self.budget
    }

    /// Returns how many distinct null or dangling links `->` met.
    pub(crate) const fn unresolved(&self) -> usize {
        self.unresolved.len()
    }

    /// Counts the link stored at the address `at`, which `->` cannot follow, once however many
    /// paths meet it.
    pub(crate) fn note_unresolved(&mut self, at: usize) {
        if !self.unresolved.contains(&at) {
            self.unresolved.push(at);
        }
    }

    /// Charges array elements against the current traversal budget.
    pub(crate) fn charge(&mut self, elements: usize) -> Result<(), ValueError> {
        self.budget = self
            .budget
            .checked_sub(elements)
            .ok_or(ValueError::PathTooWide { limit: self.limit })?;
        Ok(())
    }
}
