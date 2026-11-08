// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::Container;
use p4kx_model::ModelChunk;
use schemars::JsonSchema;
use serde::Serialize;

use crate::{ChunkInfo, DecimalU64};

/// Version of the source chunk-table projection, independent of model container versions.
pub const MODEL_CHUNK_SOURCE_INFO_VERSION: u8 = 1;

/// One retained chunk-table observation without a raw-publication or complete-model claim.
#[derive(Debug, Serialize, JsonSchema)]
#[non_exhaustive]
pub struct ModelChunkSourceInfo {
    /// Metadata contract version.
    #[schemars(extend("const" = MODEL_CHUNK_SOURCE_INFO_VERSION))]
    pub schema_version: u8,
    /// Existing logical file role, original table ordinal, type and coarse reader usage.
    pub chunk: ChunkInfo,
    /// Parsed physical table fields, independent of payload interpretation.
    pub layout: ModelChunkLayoutInfo,
}

/// Physical chunk-table facts with explicit normalized and inferred meanings.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = crate::projection_schema::require_properties)]
#[schemars(extend("allOf" = [
    {
        "if": {"properties": {"container": {"const": "#ivo"}}, "required": ["container"]},
        "then": {"properties": {
            "id": {"type": "null"}, "byte_order": {"const": "little"},
            "span": {"properties": {"basis": {"const": "derived-ivo-next-boundary-including-padding"}}, "required": ["basis"]}
        }}
    },
    {
        "if": {"properties": {"container": {"const": "CrCh"}}, "required": ["container"]},
        "then": {"properties": {
            "id": {"type": "integer"}, "version": {"maximum": 32767},
            "span": {"properties": {"basis": {"const": "declared-crch-size"}}, "required": ["basis"]}
        }}
    }
]))]
#[non_exhaustive]
pub struct ModelChunkLayoutInfo {
    /// Actual supported container signature, not an inferred header version.
    #[schemars(extend("enum" = ["#ivo", "CrCh"]))]
    pub container: &'static str,
    /// Parsed layout version without the `CrCh` byte-order flag.
    #[schemars(range(max = u32::MAX))]
    pub version: u32,
    /// Declared chunk ID, explicitly null in `#ivo`.
    #[schemars(range(max = u32::MAX))]
    pub id: Option<u32>,
    /// Data byte order, without rewriting or endian-converting retained bytes.
    #[schemars(extend("enum" = ["little", "big"]))]
    pub byte_order: &'static str,
    /// Declared offset in this chunk's owning decoded file.
    pub offset: DecimalU64,
    /// The span and its exact source or inference basis.
    pub span: ModelChunkSpanInfo,
}

/// Chunk byte-span length with the existing container reader's precise basis.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "basis", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ModelChunkSpanInfo {
    /// Length directly declared by the `CrCh` chunk table.
    DeclaredCrchSize {
        /// Declared byte length.
        bytes: DecimalU64,
    },
    /// Length to the next greater offset, table boundary or file end, including padding.
    DerivedIvoNextBoundaryIncludingPadding {
        /// Existing reader's inferred byte span.
        bytes: DecimalU64,
    },
}

impl ModelChunkSourceInfo {
    /// Projects an actual model chunk observation without reading or publishing any bytes.
    #[must_use]
    pub fn of(chunk: &ModelChunk) -> Self {
        let layout = chunk.layout;
        let container = chunk.chunk_type.container();
        let bytes = DecimalU64::new(layout.size);
        let span = match container {
            Container::Ivo => ModelChunkSpanInfo::DerivedIvoNextBoundaryIncludingPadding { bytes },
            Container::CrCh => ModelChunkSpanInfo::DeclaredCrchSize { bytes },
        };
        Self {
            schema_version: MODEL_CHUNK_SOURCE_INFO_VERSION,
            chunk: ChunkInfo::of(chunk),
            layout: ModelChunkLayoutInfo {
                container: container.name(),
                version: layout.version,
                id: layout.id,
                byte_order: layout.byte_order.name(),
                offset: DecimalU64::new(layout.offset),
                span,
            },
        }
    }
}
