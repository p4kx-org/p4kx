// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{LoadoutFileError, LoadoutFileSource};

/// One actual file-read outcome paired with its original admission.
#[derive(Debug)]
#[must_use]
pub struct LoadoutFileRead {
    source: LoadoutFileSource,
    outcome: Result<Vec<u8>, LoadoutFileError>,
}

impl LoadoutFileRead {
    /// Pairs a generic provider's configured path and outcome without Archive authority.
    pub fn unbound(path: &str, outcome: Result<Vec<u8>, LoadoutFileError>) -> Self {
        Self::new(LoadoutFileSource::unbound(path), outcome)
    }

    pub(crate) const fn new(
        source: LoadoutFileSource,
        outcome: Result<Vec<u8>, LoadoutFileError>,
    ) -> Self {
        Self { source, outcome }
    }

    /// Borrows the original admission of this operation.
    #[must_use]
    pub const fn source(&self) -> &LoadoutFileSource {
        &self.source
    }

    /// Borrows the exact completed read outcome.
    ///
    /// # Errors
    ///
    /// Returns the original file-read failure.
    pub fn outcome(&self) -> Result<&[u8], &LoadoutFileError> {
        self.outcome.as_deref()
    }

    /// Consumes the operation and returns its exact read outcome.
    ///
    /// # Errors
    ///
    /// Returns the original file-read failure.
    pub fn into_result(self) -> Result<Vec<u8>, LoadoutFileError> {
        self.outcome
    }

    /// Moves the unchanged source and its original outcome into the intrinsic reader.
    pub(crate) fn into_parts(self) -> (LoadoutFileSource, Result<Vec<u8>, LoadoutFileError>) {
        (self.source, self.outcome)
    }
}
