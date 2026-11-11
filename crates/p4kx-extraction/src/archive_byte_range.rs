// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::MetadataRange;
use schemars::JsonSchema;
use serde::Serialize;

use crate::DecimalU64;

/// Exact half-open extent in immediately owning source coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[non_exhaustive]
pub struct ArchiveByteRange {
    /// Original start measured from this Archive source, not the backing root file.
    pub offset: DecimalU64,
    /// Number of bytes, including an explicitly empty extent.
    pub length: DecimalU64,
}

impl ArchiveByteRange {
    /// Projects one checked reader extent without changing its coordinate basis.
    #[must_use]
    pub const fn of(range: MetadataRange) -> Self {
        Self {
            offset: DecimalU64::new(range.offset()),
            length: DecimalU64::new(range.len()),
        }
    }
}
