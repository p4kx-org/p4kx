// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::OnceLock;

use crate::{LoadoutDocumentKind, LoadoutFileError, LoadoutFileSource};

/// Retained intrinsic outcomes of one original loadout document operation.
#[derive(Debug, Clone)]
pub struct LoadoutFileObservation {
    source: LoadoutFileSource,
    kind: LoadoutDocumentKind,
    read: Result<(), LoadoutFileError>,
    projection: OnceLock<Result<(), String>>,
}

impl LoadoutFileObservation {
    pub(crate) const fn new(
        source: LoadoutFileSource,
        kind: LoadoutDocumentKind,
        read: Result<(), LoadoutFileError>,
    ) -> Self {
        Self {
            source,
            kind,
            read,
            projection: OnceLock::new(),
        }
    }

    pub(crate) fn projected(&self, outcome: Result<(), &str>) -> &Result<(), String> {
        self.projection
            .get_or_init(|| outcome.map_err(str::to_owned))
    }

    /// Borrows the original admission retained by the first operation.
    #[must_use]
    pub const fn source(&self) -> &LoadoutFileSource {
        &self.source
    }

    /// Returns the intrinsic projection requested by the original reader.
    #[must_use]
    pub const fn kind(&self) -> LoadoutDocumentKind {
        self.kind
    }

    /// Borrows the original read outcome without repeating the operation.
    ///
    /// # Errors
    ///
    /// Returns the original file-read failure.
    pub const fn read_outcome(&self) -> Result<(), &LoadoutFileError> {
        self.read.as_ref().copied()
    }

    /// Borrows the actual parse outcome, absent when the document was not projected.
    #[must_use]
    pub fn projection_outcome(&self) -> Option<Result<(), &str>> {
        self.projection
            .get()
            .map(|outcome| outcome.as_ref().copied().map_err(String::as_str))
    }
}
