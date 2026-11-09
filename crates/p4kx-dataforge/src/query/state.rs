// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::ValueError;
use crate::query::MAX_PATH_VALUES;

/// The array budget and unresolved links shared by the columns of one Record row.
#[derive(Debug)]
pub(crate) struct WalkState {
    budget: usize,
    unresolved: Vec<usize>,
}

impl WalkState {
    /// Returns the state of a Record no path has read yet.
    pub(crate) const fn new() -> Self {
        Self {
            budget: MAX_PATH_VALUES,
            unresolved: Vec::new(),
        }
    }

    /// Readies the state for the next Record, keeping its buffer.
    pub(crate) fn reset(&mut self) {
        self.budget = MAX_PATH_VALUES;
        self.unresolved.clear();
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

    /// Counts `elements` more array elements against [`MAX_PATH_VALUES`].
    pub(crate) fn charge(&mut self, elements: usize) -> Result<(), ValueError> {
        self.budget = self
            .budget
            .checked_sub(elements)
            .ok_or(ValueError::PathTooWide {
                limit: MAX_PATH_VALUES,
            })?;
        Ok(())
    }
}
