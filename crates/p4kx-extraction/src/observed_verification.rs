// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::Entry;

use crate::VerificationReport;

/// A verification report bound to the original selection and its outcomes.
#[derive(Debug)]
pub struct ObservedVerification<'entry> {
    pub(crate) report: VerificationReport,
    pub(crate) entries: &'entry [&'entry Entry],
    pub(crate) positions: VerificationPositions,
}

#[derive(Debug, Default)]
pub(crate) struct VerificationPositions {
    pub(crate) failed: Vec<usize>,
    pub(crate) cancelled: Vec<usize>,
}

impl<'entry> ObservedVerification<'entry> {
    /// Returns the report of the observed run.
    #[must_use]
    pub const fn report(&self) -> &VerificationReport {
        &self.report
    }

    /// Returns each selected Entry and its outcome in selection order.
    pub fn observations(
        &self,
    ) -> impl ExactSizeIterator<Item = VerificationObservation<'entry>> + '_ {
        let mut failures = self.positions.failed.iter().enumerate().peekable();
        let mut cancelled = self.positions.cancelled.iter().peekable();
        self.entries
            .iter()
            .enumerate()
            .map(move |(position, &entry)| {
                let outcome = if let Some((failure_index, _)) =
                    failures.next_if(|item| *item.1 == position)
                {
                    VerificationOutcome::Failed { failure_index }
                } else if cancelled
                    .next_if(|candidate| **candidate == position)
                    .is_some()
                {
                    VerificationOutcome::Cancelled
                } else {
                    VerificationOutcome::Verified
                };
                VerificationObservation {
                    entry,
                    position,
                    outcome,
                }
            })
    }
}

/// The physical selected Entry and its verification outcome.
#[derive(Debug, Clone, Copy)]
pub struct VerificationObservation<'entry> {
    entry: &'entry Entry,
    position: usize,
    outcome: VerificationOutcome,
}

impl<'entry> VerificationObservation<'entry> {
    /// Returns the original Entry reference.
    #[must_use]
    pub const fn entry(self) -> &'entry Entry {
        self.entry
    }

    /// Returns the zero-based position within the supplied selection.
    #[must_use]
    pub const fn position(self) -> usize {
        self.position
    }

    /// Returns the actual verification outcome.
    #[must_use]
    pub const fn outcome(self) -> VerificationOutcome {
        self.outcome
    }
}

/// The outcome of checking one selected Entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum VerificationOutcome {
    /// The verifier read the Entry to completion and all enabled checksum checks passed.
    Verified,
    /// The Entry could not be read or its data did not match the checksum.
    Failed {
        /// The position of this failure in the same report's failure list.
        failure_index: usize,
    },
    /// The run stopped before the Entry's verification returned an outcome.
    Cancelled,
}
