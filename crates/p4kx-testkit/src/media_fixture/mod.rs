// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Deterministic test-fixture selection and complete per-case coverage reports.

mod report;
mod select;

pub use report::{CaseOutcome, CaseResult, CoverageError, RunReport};
pub use select::{Candidate, Choice, SelectionError, SelectionReason, select};

#[cfg(test)]
mod tests;
