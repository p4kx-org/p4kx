// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use thiserror::Error;

use crate::ValueError;

/// A demanded value could not be read to decide which Records a Query selects.
#[derive(Debug, Error)]
#[error("cannot decide comparison {} for Record {record_index} at `{record_path}`", .comparison_index.saturating_add(1))]
pub struct SelectionError {
    record_index: u32,
    record_path: String,
    comparison_index: usize,
    #[source]
    source: ValueError,
}

impl SelectionError {
    /// Captures the demanded failure for one Record and comparison.
    pub(super) fn new(
        record_index: u32,
        record_path: &str,
        comparison_index: usize,
        source: ValueError,
    ) -> Self {
        Self {
            record_index,
            record_path: record_path.to_owned(),
            comparison_index,
            source,
        }
    }

    /// Returns the definition index of the Record whose condition could not be decided.
    #[must_use]
    pub const fn record_index(&self) -> u32 {
        self.record_index
    }

    /// Returns the record path of the Record whose condition could not be decided.
    #[must_use]
    pub fn record_path(&self) -> &str {
        &self.record_path
    }

    /// Returns the zero-based comparison index in condition order.
    #[must_use]
    pub const fn comparison_index(&self) -> usize {
        self.comparison_index
    }

    /// Returns the value-reading error that prevents the decision.
    #[must_use]
    pub const fn value_error(&self) -> &ValueError {
        &self.source
    }
}
