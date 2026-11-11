// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::{Arc, OnceLock};

use crate::{LoadoutDocumentKind, LoadoutFileError, LoadoutFileSource};

/// Retained intrinsic outcomes and identity of one original loadout document operation.
#[derive(Debug, Clone)]
pub struct LoadoutFileObservation {
    operation: Arc<Operation>,
}

/// An opaque routing key for an original operation retained by the caller.
///
/// A lookup still checks the actual original operation.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct LoadoutFileOperationKey(usize);

#[derive(Debug)]
struct Operation {
    source: LoadoutFileSource,
    kind: LoadoutDocumentKind,
    read: Result<(), LoadoutFileError>,
    projection: OnceLock<Result<(), String>>,
}

impl LoadoutFileObservation {
    pub(crate) fn new(
        source: LoadoutFileSource,
        kind: LoadoutDocumentKind,
        read: Result<(), LoadoutFileError>,
    ) -> Self {
        Self {
            operation: Arc::new(Operation {
                source,
                kind,
                read,
                projection: OnceLock::new(),
            }),
        }
    }

    pub(crate) fn projected(&self, outcome: Result<(), &str>) -> &Result<(), String> {
        self.operation
            .projection
            .get_or_init(|| outcome.map_err(str::to_owned))
    }

    /// Returns whether both handles retain the same original document operation.
    #[must_use]
    pub fn is_same_operation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.operation, &other.operation)
    }

    /// Returns the routing key of this retained operation.
    #[must_use]
    pub fn operation_key(&self) -> LoadoutFileOperationKey {
        LoadoutFileOperationKey(Arc::as_ptr(&self.operation).addr())
    }

    /// Borrows the original admission retained by the first operation.
    #[must_use]
    pub fn source(&self) -> &LoadoutFileSource {
        &self.operation.source
    }

    /// Returns the intrinsic projection requested by the original reader.
    #[must_use]
    pub fn kind(&self) -> LoadoutDocumentKind {
        self.operation.kind
    }

    /// Borrows the original read outcome without repeating the operation.
    ///
    /// # Errors
    ///
    /// Returns the original file-read failure.
    pub fn read_outcome(&self) -> Result<(), &LoadoutFileError> {
        self.operation.read.as_ref().copied()
    }

    /// Borrows the actual parse outcome, absent when the document was not projected.
    #[must_use]
    pub fn projection_outcome(&self) -> Option<Result<(), &str>> {
        self.operation
            .projection
            .get()
            .map(|outcome| outcome.as_ref().copied().map_err(String::as_str))
    }
}
