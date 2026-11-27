// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Guid, Record};

/// A Record the catalog build could not read; the entry it would have produced is left out.
///
/// A ship whose loadout cannot be resolved stays in `ships` without a loadout, and its `reason`
/// starts with `loadout: `.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogFailure {
    /// The GUID of the Record.
    pub guid: Guid,
    /// The record path, or an empty text when it cannot be read.
    pub record_path: String,
    /// Why the Record cannot be read.
    pub reason: String,
}

impl CatalogFailure {
    /// Returns the failure of `record`, which cannot be read because of `error`.
    pub(crate) fn of(record: &Record<'_>, error: &dyn Error) -> Self {
        let reason = crate::failure_text::render(error);
        Self {
            guid: record.guid(),
            record_path: record.record_path().map_or_default(str::to_owned),
            reason,
        }
    }
}
