// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::MAX_PART_BYTES;

use crate::MediaError;

/// A portable lower-case ASCII alphanumeric encoding suffix without its dot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactExtension<'a>(&'a str);

impl<'a> ArtifactExtension<'a> {
    /// Validates the suffix together with its SHA-256 filename prefix.
    ///
    /// # Errors
    /// Returns a contract failure for an empty, noncanonical or overlong suffix.
    pub fn new(value: &'a str) -> Result<Self, MediaError> {
        const SHA256_HEX_BYTES: usize = 32 * 2;
        const SEPARATOR_BYTES: usize = 1;
        if value.is_empty()
            || value.len() > MAX_PART_BYTES - SHA256_HEX_BYTES - SEPARATOR_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        {
            return Err(MediaError::Contract(
                "artifact extension is not a portable canonical suffix".into(),
            ));
        }
        Ok(Self(value))
    }

    /// Returns the validated suffix without its dot.
    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}

#[cfg(test)]
#[path = "artifact_extension_tests.rs"]
mod tests;
