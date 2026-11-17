// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use crate::export_diagnostic::ExportDiagnosticCode;
use crate::record_read::ReadError;
use crate::source_fields::FieldError;

/// One canonical typed field note at its original selected Generator.
#[derive(Debug)]
pub struct ContractGeneratorProjectionNote {
    record_index: u32,
    note: FieldError,
}

impl ContractGeneratorProjectionNote {
    pub(super) const fn new(record_index: u32, note: FieldError) -> Self {
        Self { record_index, note }
    }

    /// The original physical Generator Record index.
    #[must_use]
    pub const fn record_index(&self) -> u32 {
        self.record_index
    }

    /// The canonical semantic diagnostic category.
    #[must_use]
    pub fn code(&self) -> ExportDiagnosticCode {
        crate::export_projection_fields::code(self.note.description().issue)
    }

    /// The original declaring source type when known.
    #[must_use]
    pub fn source_type(&self) -> Option<&str> {
        self.note.description().source_type
    }

    /// The exact original field path.
    #[must_use]
    pub fn field(&self) -> &str {
        self.note.description().field
    }

    /// Whether the note describes an incomplete typed interpretation.
    #[must_use]
    pub fn blocks_projection(&self) -> bool {
        self.note.description().issue.blocks_projection()
    }

    /// The complete original typed field error.
    #[must_use]
    pub fn error(&self) -> &(dyn Error + 'static) {
        &self.note
    }
}

/// An unsuccessful mission-item interpretation with its complete source still retained.
#[derive(Debug)]
pub struct ContractGeneratorProjectionFailure {
    record_index: u32,
    error: ReadError,
}

impl ContractGeneratorProjectionFailure {
    pub(super) fn new(record_index: u32, error: ReadError) -> Self {
        Self {
            record_index,
            error,
        }
    }

    /// The original physical Generator Record index.
    #[must_use]
    pub const fn record_index(&self) -> u32 {
        self.record_index
    }

    /// The original interpretation failure.
    #[must_use]
    pub fn error(&self) -> &(dyn Error + Send + Sync + 'static) {
        self.error.as_ref()
    }
}
