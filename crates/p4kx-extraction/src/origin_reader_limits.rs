// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

/// Explicit finite policy for reading origin metadata without changing producer limits.
#[derive(Debug, Clone, Copy)]
pub struct OriginReaderLimits {
    /// Maximum addressed fields in one companion.
    pub fields: usize,
    /// Maximum logical storage of copied paths, references and fixed carrier values.
    pub metadata_bytes: usize,
    /// Maximum UTF-8 bytes in one escaped field path.
    pub path_bytes: usize,
    /// Maximum contributing references in one field origin.
    pub field_uses: usize,
    /// Maximum UTF-8 bytes in one document-local use reference.
    pub reference_bytes: usize,
}

pub(crate) struct ReadBudget(Cell<usize>);

impl ReadBudget {
    pub(crate) const fn new(bytes: usize) -> Self {
        Self(Cell::new(bytes))
    }

    pub(crate) fn charge<E: serde::de::Error>(&self, bytes: usize) -> Result<(), E> {
        let remaining = self
            .0
            .get()
            .checked_sub(bytes)
            .ok_or_else(|| E::custom("origin metadata exceeds the reader byte policy"))?;
        self.0.set(remaining);
        Ok(())
    }
}
