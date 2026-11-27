// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::TryReserveError;

/// A path index whose own storage or physical source inventory is invalid.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PathIndexError {
    /// Index storage could not be allocated.
    #[error("cannot allocate path-index keys: {0}")]
    Allocation(#[source] TryReserveError),
    /// A source position does not fit the physical index width.
    #[error("path-index position {position} exceeds the physical index width")]
    IndexWidth {
        /// Original source position.
        position: usize,
    },
    /// The keys do not belong to the source Entry inventory.
    #[error("path-index keys do not match their source inventory")]
    SourceMismatch,
}
