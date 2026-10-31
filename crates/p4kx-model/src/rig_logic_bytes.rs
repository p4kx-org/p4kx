// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::sync::Arc;

/// The raw bytes of a skin's `RigLogic` chunk, shared by every clone.
///
/// Two values are equal when their bytes are equal. `Debug` prints the length only, so a model
/// stays printable.
#[derive(Clone, PartialEq, Eq)]
pub struct RigLogicBytes(Arc<[u8]>);

impl RigLogicBytes {
    /// Copies `bytes` into a shared buffer.
    #[must_use]
    pub fn new(bytes: &[u8]) -> Self {
        Self(Arc::from(bytes))
    }

    /// Returns the bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// Returns the number of bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns whether there are no bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for RigLogicBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "RigLogicBytes({} bytes)", self.len())
    }
}
