// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// The decoder selected for an Entry's decrypted stored bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectiveEncoding {
    /// The Entry uses ZIP method 0 without decompression.
    Stored,
    /// The Entry uses a raw deflate decoder.
    Deflate,
    /// CIG method 100 contains a Zstandard frame.
    Zstandard,
    /// CIG method 100 contains direct bytes without Zstandard magic.
    PassThrough,
    /// The decoder has not selected an encoding or its prefix could not be read.
    Undetermined,
}
