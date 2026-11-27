// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    ArchiveMetadataVersion, DecimalU64, EntryMetadataInfo, EntryRecordFormatInfo,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::archive_raw::ArchiveRawSpan;

/// One original directory record, including unadmitted folder records.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[non_exhaustive]
pub struct ArchiveDirectoryRecord {
    /// Source-layout payload contract.
    pub version: ArchiveMetadataVersion,
    /// Observed cursor position, distinguished from an unknown recovered original ordinal.
    pub position: DecimalU64,
    /// Original directory ordinal, explicitly unavailable after recovery.
    pub original_directory_ordinal: Option<DecimalU64>,
    /// Existing physical index, absent for omitted folder records.
    pub admitted_entry_index: Option<DecimalU64>,
    /// Record format actually established by the reader.
    pub format: EntryRecordFormatInfo,
    /// Shared CLI/MCP/Media physical Entry metadata, absent for omitted records.
    pub metadata: Option<EntryMetadataInfo>,
    /// Complete original record bytes.
    pub raw: ArchiveRawSpan,
    /// Original ZIP name bytes, preserving invalid UTF-8 and empty names.
    pub name: Option<ArchiveRawSpan>,
    /// Original ordered extra section, including malformed remaining bytes.
    pub extras: Option<ArchiveRawSpan>,
    /// Original ZIP comment, preserving an explicitly empty section.
    pub comment: Option<ArchiveRawSpan>,
    /// All 128 authored v2 signature bytes without an authentication claim.
    pub signature: Option<ArchiveRawSpan>,
    /// All 32 authored v2 digest-slot bytes, including zeros, without verification inference.
    pub stored_digest: Option<ArchiveRawSpan>,
}

/// Exact framing coverage of an ordered original extra field.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ArchiveExtraState {
    /// Header and declared body are available.
    Complete,
    /// The remaining section ends inside its header.
    ShortHeader,
    /// The declared total length is shorter than its four-byte header.
    LengthBeforeHeader,
    /// Only an original body prefix is available.
    TruncatedBody,
}

/// One ordered extra retaining unknown and duplicate IDs without interpretation.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[non_exhaustive]
pub struct ArchiveExtraData {
    /// Source-layout payload contract.
    pub version: ArchiveMetadataVersion,
    /// Owning record's observed cursor position.
    pub record_position: DecimalU64,
    /// Original field position; duplicate IDs retain independent positions.
    pub ordinal: DecimalU64,
    /// Original ID word when available.
    #[schemars(range(min = 0, max = u16::MAX))]
    pub id: Option<u16>,
    /// Original total length including the four-byte header.
    #[schemars(range(min = 0, max = u16::MAX))]
    pub declared_total_length: Option<u16>,
    /// Framing validity independently of ID semantics.
    pub state: ArchiveExtraState,
    /// Every available original byte of this field or malformed tail.
    pub raw: ArchiveRawSpan,
    /// Original body or partial prefix, absent when no body start is established.
    pub body: Option<ArchiveRawSpan>,
}
