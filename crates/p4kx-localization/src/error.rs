// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use thiserror::Error;

/// A `global.ini` cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum LocalizationError {
    /// The source byte count exceeds the caller's budget.
    #[error("the global.ini has {size} bytes, more than the limit of {limit} bytes")]
    TooLarge {
        /// The size of the file in bytes.
        size: u64,
        /// The caller's source byte budget.
        limit: u64,
    },
    /// The decoded UTF-8 byte count exceeds the caller's budget.
    #[error(
        "the decoded global.ini needs at least {size} UTF-8 bytes, more than the limit of {limit} bytes"
    )]
    DecodedTooLarge {
        /// The size of the decoded text or its first over-budget prefix.
        size: u64,
        /// The caller's decoded UTF-8 byte budget.
        limit: u64,
    },
    /// The memory to decode the file to UTF-8 cannot be reserved.
    #[error("cannot reserve the memory to decode the {size} bytes of the global.ini")]
    DecodeOutOfMemory {
        /// The size of the file in bytes.
        size: u64,
    },
    /// The memory for the index of the file's keys cannot be reserved.
    #[error("cannot reserve the memory to index {keys} keys of the global.ini")]
    OutOfMemory {
        /// The number of keys the reservation was for.
        keys: usize,
    },
    /// The file holds more distinct keys than the caller's budget.
    #[error("the global.ini holds more than {limit} keys")]
    TooManyKeys {
        /// The caller's distinct key budget.
        limit: usize,
    },
}
