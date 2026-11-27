// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::format_error::FormatError;

/// Refuses an Entry count above `max_entries`.
pub(crate) fn check_entry_count(count: u64, max_entries: Option<u64>) -> Result<(), FormatError> {
    match max_entries {
        Some(max) if count > max => Err(FormatError::LimitExceeded(format!(
            "it holds {count} Entries, more than {max}"
        ))),
        _ => Ok(()),
    }
}
