// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::metadata_range::MetadataRange;

pub(crate) const ZIP_PROGRESS_BYTES: usize = 12;
pub(crate) const V2_PROGRESS_BYTES: usize = 8;
pub(crate) const FREE_RECORD_BYTES: usize = 16;

/// A failure to locate declared installation tables without guessing their bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MetadataTableProblem {
    /// Count and record-size arithmetic overflowed.
    Overflow,
    /// The declared tables cannot fit before the directory.
    OutsideSource,
}

/// Checked installation/free extents or a typed failed declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MetadataTables {
    /// Both tables fit in their immediately owning source.
    Located {
        /// Checked installation-progress table extent.
        progress: MetadataRange,
        /// Checked free-range table extent.
        free: MetadataRange,
    },
    /// The raw declarations remain available but no table location is proved.
    Invalid(MetadataTableProblem),
}

impl MetadataTables {
    pub(crate) fn before(
        directory: u64,
        count: u64,
        free_count: u64,
        progress_width: u64,
        source_len: u64,
    ) -> Self {
        let sizes = count
            .checked_mul(progress_width)
            .zip(free_count.checked_mul(FREE_RECORD_BYTES as u64));
        let Some((progress_len, free_len)) = sizes else {
            return Self::Invalid(MetadataTableProblem::Overflow);
        };
        let Some(total) = progress_len.checked_add(free_len) else {
            return Self::Invalid(MetadataTableProblem::Overflow);
        };
        let Some(start) = directory.checked_sub(total) else {
            return Self::Invalid(MetadataTableProblem::OutsideSource);
        };
        Self::at(start, progress_len, free_len, source_len)
    }

    pub(crate) fn at(start: u64, progress_len: u64, free_len: u64, source_len: u64) -> Self {
        let Some(free_start) = start.checked_add(progress_len) else {
            return Self::Invalid(MetadataTableProblem::Overflow);
        };
        if free_start.checked_add(free_len).is_none() {
            return Self::Invalid(MetadataTableProblem::Overflow);
        }
        match (
            MetadataRange::checked(start, progress_len, source_len),
            MetadataRange::checked(free_start, free_len, source_len),
        ) {
            (Ok(progress), Ok(free)) => Self::Located { progress, free },
            _ => Self::Invalid(MetadataTableProblem::OutsideSource),
        }
    }
}
