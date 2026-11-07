// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Arc;

use crate::{DataForgeError, Record, RecordTree};

/// Observes an actual canonical decode before its tree reaches projection helpers.
pub trait CanonicalReadObserver: Send + Sync {
    /// Returns `Break` to stop the caller with `DataForgeError::ObservationStopped`.
    fn observe(
        &self,
        record: &Record<'_>,
        outcome: Result<&RecordTree<'_>, &DataForgeError>,
    ) -> ControlFlow<()>;
}

pub(crate) struct ReadObserver(pub(crate) Arc<dyn CanonicalReadObserver>);

impl std::fmt::Debug for ReadObserver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReadObserver")
            .finish_non_exhaustive()
    }
}
