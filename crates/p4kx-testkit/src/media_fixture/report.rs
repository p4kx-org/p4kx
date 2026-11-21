// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fmt::{self, Display};

/// The coverage established by one completed case callback.
#[derive(Debug)]
#[non_exhaustive]
pub enum CaseOutcome<T> {
    /// A selected fixture completed conversion and verification.
    Completed(T),
    /// No source met the documented capability requirements.
    Absent(String),
}

/// One case with its original producer outcome retained internally.
#[derive(Debug)]
#[non_exhaustive]
pub struct CaseResult<T> {
    /// The caller's technical case label.
    pub case: String,
    /// The completed, absent or failed outcome.
    pub outcome: Result<CaseOutcome<T>, Box<dyn Error>>,
}

/// Aggregate coverage when a run fails or completes no meaningful case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct CoverageError {
    /// Cases whose selected producer or verification failed.
    pub failed: usize,
    /// Cases that completed conversion and verification.
    pub completed: usize,
    /// Cases with no compatible available source.
    pub absent: usize,
}

impl Display for CoverageError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            "media coverage: {} completed, {} absent, {} failed",
            self.completed, self.absent, self.failed
        )
    }
}

impl Error for CoverageError {}

/// Executes every case and retains failures without formatting source data.
#[derive(Debug)]
pub struct RunReport<T> {
    cases: Vec<CaseResult<T>>,
}

impl<T> Default for RunReport<T> {
    fn default() -> Self {
        Self { cases: Vec::new() }
    }
}

impl<T> RunReport<T> {
    /// Runs a case while preserving earlier failures and permitting later callbacks.
    pub fn run(
        &mut self,
        case: impl Into<String>,
        execute: impl FnOnce() -> Result<CaseOutcome<T>, Box<dyn Error>>,
    ) {
        self.cases.push(CaseResult {
            case: case.into(),
            outcome: execute(),
        });
    }

    /// Returns every source-bearing outcome in registration order.
    #[must_use]
    pub fn cases(&self) -> &[CaseResult<T>] {
        &self.cases
    }

    fn counts(&self) -> CoverageError {
        let mut counts = CoverageError {
            failed: 0,
            completed: 0,
            absent: 0,
        };
        for case in &self.cases {
            match &case.outcome {
                Ok(CaseOutcome::Completed(_)) => counts.completed += 1,
                Ok(CaseOutcome::Absent(_)) => counts.absent += 1,
                Err(_) => counts.failed += 1,
            }
        }
        counts
    }

    /// Requires at least one completed case and no failed selected conversions.
    ///
    /// # Errors
    ///
    /// Returns aggregate counts for failed or entirely absent coverage.
    pub fn verify(&self) -> Result<(), CoverageError> {
        let counts = self.counts();
        if counts.failed == 0 && counts.completed > 0 {
            Ok(())
        } else {
            Err(counts)
        }
    }
}

impl<T> Display for RunReport<T> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for case in &self.cases {
            let status = match &case.outcome {
                Ok(CaseOutcome::Completed(_)) => "completed",
                Ok(CaseOutcome::Absent(_)) => "absent",
                Err(_) => "failed",
            };
            writeln!(out, "{}: {status}", case.case)?;
        }
        write!(out, "{}", self.counts())
    }
}
