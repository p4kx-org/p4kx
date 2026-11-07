// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::MediaError;

/// Bounds applied before reading or writing the next unit of work.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct Limits {
    /// Bytes hashed from the complete root file.
    pub root_bytes: u64,
    /// Physical Entries examined during traversal, including filtered Entries.
    pub sources: u64,
    /// Decoded bytes retained or used to open nested containers.
    pub source_bytes: u64,
    /// Decoded bytes of one Entry.
    pub entry_bytes: u64,
    /// Metadata and subresource records over all providers.
    pub records: u64,
    /// Bytes in one serialised record, including its newline.
    pub record_bytes: u64,
    /// Bytes in one typed binary chunk.
    pub chunk_bytes: u64,
    /// Bytes written to generation artifacts, excluding the separately bounded index.
    pub output_bytes: u64,
    /// Ordered container chain length.
    pub depth: u32,
    /// Maximum JSON value nesting, including the record envelope.
    pub value_depth: u32,
    /// Reference queries across the generation.
    pub references: u64,
    /// Actual candidates retained for one complete reference lookup.
    pub reference_candidates: u32,
}

impl Default for Limits {
    fn default() -> Self {
        const GIB: u64 = 1 << 30;
        Self {
            root_bytes: 4_096 * GIB,
            sources: 10_000_000,
            source_bytes: 8_192 * GIB,
            entry_bytes: 8 * GIB,
            records: 10_000_000,
            record_bytes: 1 << 20,
            chunk_bytes: 64 << 20,
            output_bytes: 8_192 * GIB,
            depth: 8,
            value_depth: 128,
            references: 10_000_000,
            reference_candidates: 128,
        }
    }
}

impl Limits {
    pub(crate) fn validate(self) -> Result<(), MediaError> {
        if u64::from(self.depth) > p4kx_extraction::MAX_CONTAINER_DEPTH as u64 {
            return Err(MediaError::Contract(
                "container depth exceeds reader capacity".into(),
            ));
        }
        if self.value_depth == 0 || self.value_depth > 512 {
            return Err(MediaError::Contract(
                "value depth must be between one and 512".into(),
            ));
        }
        for (budget, value) in [
            ("root_bytes", self.root_bytes),
            ("sources", self.sources),
            ("source_bytes", self.source_bytes),
            ("entry_bytes", self.entry_bytes),
            ("records", self.records),
            ("record_bytes", self.record_bytes),
            ("chunk_bytes", self.chunk_bytes),
            ("output_bytes", self.output_bytes),
            ("references", self.references),
            ("reference_candidates", u64::from(self.reference_candidates)),
        ] {
            if value == 0 {
                return Err(MediaError::Limit {
                    budget,
                    limit: value,
                });
            }
        }
        Ok(())
    }
}
