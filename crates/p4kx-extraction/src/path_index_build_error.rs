// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::PathIndexError;

/// An original caller failure or a failure of the bounded physical path index.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PathIndexBuildError<E> {
    /// The caller's original cancellation, source-change or budget failure.
    #[error("path-index admission failed: {0}")]
    Interrupted(#[source] E),
    /// The index's own storage or source-inventory failure.
    #[error("path-index creation failed: {0}")]
    Index(#[from] PathIndexError),
}

impl<E> PathIndexBuildError<E> {
    /// Separates the original caller failure from the index's own failure.
    ///
    /// # Errors
    /// Returns the original typed storage or physical-inventory error.
    pub fn into_cause(self) -> Result<E, PathIndexError> {
        match self {
            Self::Interrupted(error) => Ok(error),
            Self::Index(error) => Err(error),
        }
    }
}
