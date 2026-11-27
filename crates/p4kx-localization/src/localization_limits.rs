// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::LocalizationError;

/// Optional budgets for decoding and indexing one localisation file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct LocalizationLimits {
    source_bytes: Option<u64>,
    decoded_bytes: Option<u64>,
    keys: Option<usize>,
}

impl LocalizationLimits {
    /// Reads all representable input without a caller-imposed quota.
    pub const UNLIMITED: Self = Self {
        source_bytes: None,
        decoded_bytes: None,
        keys: None,
    };

    /// Sets the maximum number of source bytes, including a byte order mark.
    #[must_use]
    pub const fn with_source_bytes(mut self, limit: u64) -> Self {
        self.source_bytes = Some(limit);
        self
    }

    /// Sets the maximum number of decoded UTF-8 bytes, excluding the leading byte order mark.
    #[must_use]
    pub const fn with_decoded_bytes(mut self, limit: u64) -> Self {
        self.decoded_bytes = Some(limit);
        self
    }

    /// Sets the maximum number of keys after trimming, ASCII case folding and suffix handling.
    #[must_use]
    pub const fn with_keys(mut self, limit: usize) -> Self {
        self.keys = Some(limit);
        self
    }

    /// Returns the source byte budget.
    #[must_use]
    pub const fn source_bytes(self) -> Option<u64> {
        self.source_bytes
    }

    /// Returns the decoded UTF-8 byte budget.
    #[must_use]
    pub const fn decoded_bytes(self) -> Option<u64> {
        self.decoded_bytes
    }

    /// Returns the distinct key budget.
    #[must_use]
    pub const fn keys(self) -> Option<usize> {
        self.keys
    }

    pub(crate) fn check_source_bytes(self, len: usize) -> Result<(), LocalizationError> {
        let size = u64::try_from(len).unwrap_or(u64::MAX);
        if let Some(limit) = self.source_bytes
            && size > limit
        {
            return Err(LocalizationError::TooLarge { size, limit });
        }
        Ok(())
    }

    pub(crate) fn check_decoded_bytes(self, len: usize) -> Result<(), LocalizationError> {
        let size = u64::try_from(len).unwrap_or(u64::MAX);
        if let Some(limit) = self.decoded_bytes
            && size > limit
        {
            return Err(LocalizationError::DecodedTooLarge { size, limit });
        }
        Ok(())
    }
}
