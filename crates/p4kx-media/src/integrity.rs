// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Integrity algorithm, without conflating CRC checks with cryptographic hashes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Algorithm {
    /// SHA-256 cryptographic hash.
    Sha256,
    /// ZIP CRC-32 polynomial.
    Crc32,
    /// Castagnoli CRC-32C polynomial.
    Crc32c,
}

/// Exact byte scope of a computed or declared integrity value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ByteScope {
    /// Every byte streamed from the root Archive file.
    RootArchiveFile,
    /// Entry bytes before decryption and decompression.
    StoredEntryPayload,
    /// Entry bytes after decryption and decompression.
    DecodedEntryPayload,
    /// Every byte of a generated artifact file.
    ArtifactFile,
}

/// Comparison against available source-declared integrity, independent of hash computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Verification {
    /// The source has no comparable integrity declaration.
    NotAvailable,
    /// A declaration exists but its interpretation does not permit verification.
    NotPerformed,
    /// Available source checks passed while the entire Entry was retained.
    Verified,
}
