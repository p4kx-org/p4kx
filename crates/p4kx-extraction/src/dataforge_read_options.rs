// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::DataForgeLoadError;
use crate::decoded_buffer::{DecodedBuffer, MAX_ADDRESSABLE_BYTES};

/// Admission policy for one decoded DataForge payload.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct DataForgeReadOptions {
    max_decoded_bytes: Option<u64>,
}

impl DataForgeReadOptions {
    /// Limits the declared decoded input length, including the database header.
    #[must_use]
    pub const fn with_max_decoded_bytes(mut self, limit: u64) -> Self {
        self.max_decoded_bytes = Some(limit);
        self
    }

    pub(crate) fn admit(self, path: &str, size: u64) -> Result<DecodedBuffer, DataForgeLoadError> {
        if let Some(limit) = self.max_decoded_bytes.filter(|&limit| size > limit) {
            return Err(DataForgeLoadError::BudgetExceeded {
                path: path.to_owned(),
                size,
                limit,
            });
        }
        DecodedBuffer::new(size).ok_or_else(|| DataForgeLoadError::TooLarge {
            path: path.to_owned(),
            size,
            limit: MAX_ADDRESSABLE_BYTES,
        })
    }
}
