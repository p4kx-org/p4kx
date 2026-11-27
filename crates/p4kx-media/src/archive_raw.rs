// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{ArchiveByteRange, ArchiveMetadataVersion, DecimalU64};
use schemars::JsonSchema;
use serde::Serialize;

use crate::ChunkReference;
use crate::archive_section::ArchiveSection;

/// Source-local series of verified raw chunks without a new physical identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub enum ArchiveRawKey {
    /// A proved global metadata section.
    Section {
        /// Original semantic section, independent of overlap aliases.
        section: ArchiveSection,
    },
    /// One surviving record when recovery provides no global layout.
    RecoveredRecord {
        /// Stable surviving physical record position; not an invented original ordinal.
        record: DecimalU64,
    },
}

/// A proved subrange of a fully emitted source-local raw series.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[non_exhaustive]
pub struct ArchiveRawSpan {
    /// Series named by archive-raw-chunk rows of the same owning Source.
    pub series: ArchiveRawKey,
    /// Exact source-relative range covered by that series' chunk intervals.
    pub range: ArchiveByteRange,
}

/// One bounded, verified sidecar segment of an owning source's metadata.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[non_exhaustive]
pub struct ArchiveRawChunk {
    /// Source-layout payload contract.
    pub version: ArchiveMetadataVersion,
    /// Series shared by its range references.
    pub series: ArchiveRawKey,
    /// Original zero-based chunk position within this series.
    pub ordinal: DecimalU64,
    /// Exact owning-source bytes stored by the artifact.
    pub range: ArchiveByteRange,
    /// Existing typed chunk contract, hash and actual output size.
    pub chunk: ChunkReference,
}
