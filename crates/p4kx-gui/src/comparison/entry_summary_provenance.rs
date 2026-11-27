// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_comparison::{CancelToken, EntrySummaryProvenance, EntrySummaryProvenanceError};

use super::ComparisonView;

impl ComparisonView {
    /// Captures original inputs for the currently displayed Entry summary on demand.
    ///
    /// The previous display remains the source while a new Filter is running.
    /// Returns `None` on cancellation.
    ///
    /// # Errors
    ///
    /// Returns an error when the selected counts cannot be bound to their original inputs.
    pub fn entry_summary_provenance(
        &self,
        cancel: &CancelToken,
    ) -> Result<Option<EntrySummaryProvenance>, EntrySummaryProvenanceError> {
        if cancel.is_cancelled() {
            return Ok(None);
        }
        let comparison = self.matched.as_ref().map_or_else(
            || self.comparison.comparison(),
            |matched| {
                self.comparison
                    .comparison_of_filter(&matched.mask, matched.filter.as_ref())
            },
        );
        comparison.entry_summary_provenance(cancel)
    }
}
