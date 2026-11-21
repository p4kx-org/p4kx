// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{MetadataTableProblem, MetadataTables};
use schemars::JsonSchema;
use serde::Serialize;

use crate::ArchiveByteRange;

/// Cached table-location declarations without reading or validating individual rows.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub enum ArchiveTableDeclaration {
    /// Header-established table extents; row validation remains lazy.
    Located {
        /// Original progress-table extent.
        progress: ArchiveByteRange,
        /// Original free-list extent.
        free: ArchiveByteRange,
    },
    /// Source-width count arithmetic overflowed.
    Overflow,
    /// Declared tables cannot fit inside their owning source.
    OutsideSource,
    /// The reader exposes an uninterpreted table-location state.
    Unknown,
}

impl ArchiveTableDeclaration {
    /// Projects cached declaration state without accessing table records.
    #[must_use]
    pub const fn of(value: MetadataTables) -> Self {
        match value {
            MetadataTables::Located { progress, free } => Self::Located {
                progress: ArchiveByteRange::of(progress),
                free: ArchiveByteRange::of(free),
            },
            MetadataTables::Invalid(MetadataTableProblem::Overflow) => Self::Overflow,
            MetadataTables::Invalid(MetadataTableProblem::OutsideSource) => Self::OutsideSource,
            _ => Self::Unknown,
        }
    }
}
