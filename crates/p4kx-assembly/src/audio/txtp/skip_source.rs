// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::TxtpReport;

/// Physical selections consumed by one skipped TXTP event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct TxtpSkipSource {
    /// Position of the trigger in the export input.
    pub trigger_ordinal: usize,
    /// Position of the first resolved variant of this event in the trigger.
    pub event_variant_ordinal: usize,
    /// Position of the first failing leaf when its declared identity appears in the reason.
    pub failed_variant_ordinal: Option<usize>,
}

/// TXTP outcomes and the physical selections of their skipped events.
#[derive(Debug)]
#[non_exhaustive]
pub struct TxtpReportWithOrigins {
    /// The outcomes of rendering and publication.
    pub report: TxtpReport,
    /// The source positions in the exact order of `report.skipped`.
    pub skipped_sources: Vec<TxtpSkipSource>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct EventFailure {
    pub(super) reason: String,
    pub(super) leaf: Option<usize>,
}

impl EventFailure {
    pub(super) fn unbound(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            leaf: None,
        }
    }

    pub(super) const fn leaf(reason: String, variant: usize) -> Self {
        Self {
            reason,
            leaf: Some(variant),
        }
    }
}
