// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigins, SourceInventory};
use thiserror::Error;

use crate::report::entry_origins;
use crate::report_origins::{ReportOriginFields, ReportOriginsError};
use crate::{CancelToken, Comparison};

/// Original input contributions to the displayed Entry comparison aggregates.
#[derive(Debug)]
pub struct EntrySummaryProvenance {
    evidence: ReportOriginFields,
}

/// The observed Entry inputs could not be bound to the summary.
#[derive(Debug, Error)]
#[error(transparent)]
pub struct EntrySummaryProvenanceError(#[from] ReportOriginsError);

impl EntrySummaryProvenance {
    /// Returns the origins for the Entry counts, directories and incomparable pair count.
    #[must_use]
    pub const fn field_origins(&self) -> &FieldOrigins {
        &self.evidence.field_origins
    }

    /// Returns the physical input inventory for these field origins.
    #[must_use]
    pub const fn source_inventory(&self) -> &SourceInventory {
        &self.evidence.provenance
    }
}

impl Comparison<'_> {
    /// Captures the Entry aggregate origins on demand.
    ///
    /// Uses report field paths for counts and directories, plus `/entries/incomparableCount`.
    /// Returns `None` on cancellation.
    ///
    /// # Errors
    ///
    /// Returns an error when the aggregates cannot be bound to their original inputs.
    pub fn entry_summary_provenance(
        &self,
        cancel: &CancelToken,
    ) -> Result<Option<EntrySummaryProvenance>, EntrySummaryProvenanceError> {
        let evidence = entry_origins::capture_summary(self, cancel)?;
        Ok(evidence.map(|evidence| EntrySummaryProvenance { evidence }))
    }
}
