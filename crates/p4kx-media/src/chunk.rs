// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::artifact_placement;
use crate::chunk_copy;
use crate::{Artifact, MediaError, MediaWriter};

/// Element representation of a typed payload chunk.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum DataType {
    /// Unsigned byte.
    U8,
    /// Signed 16-bit integer.
    I16,
    /// Unsigned 16-bit integer.
    U16,
    /// Signed 32-bit integer.
    I32,
    /// Unsigned 32-bit integer.
    U32,
    /// IEEE-754 single precision, preserving every bit pattern.
    F32,
    /// IEEE-754 double precision, preserving every bit pattern.
    F64,
}

impl DataType {
    const fn width(self) -> u64 {
        match self {
            Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::F64 => 8,
        }
    }
}

/// Byte order used inside each numeric element.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ByteOrder {
    /// Least significant byte first.
    Little,
    /// Most significant byte first.
    Big,
}

/// Array traversal order without inferred coordinate or sample conventions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum DataOrder {
    /// Rows before columns.
    RowMajor,
    /// Columns before rows.
    ColumnMajor,
    /// Original sequential source order.
    Source,
}

/// Origin against which a source byte offset is measured.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum OffsetBasis {
    /// Start of the decoded Entry payload.
    Entry,
    /// Start of an explicitly identified source section.
    Section {
        /// Original zero-based section ordinal.
        index: u64,
    },
    /// Start of an explicitly identified source object.
    Object {
        /// Ordered original object ordinal path.
        indices: Vec<u64>,
    },
    /// Start of an explicitly identified nested payload.
    Payload {
        /// Stable provider namespace and original ordinal.
        namespace: String,
        /// Original payload ordinal.
        index: u64,
    },
}

/// A half-open byte range with an explicit origin.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct SourceRange {
    /// Origin of both offsets.
    pub basis: OffsetBasis,
    /// First byte included.
    pub start: u64,
    /// First byte excluded.
    pub end_exclusive: u64,
}

/// Typed element range written as one bounded binary artifact.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct ChunkLayout {
    /// Element representation.
    pub dtype: DataType,
    /// Element byte order.
    pub byte_order: ByteOrder,
    /// Original array traversal order.
    pub order: DataOrder,
    /// Number of elements in this chunk.
    pub count: u64,
    /// First element index in the complete source array.
    pub first: u64,
    /// First element index excluded from this chunk.
    pub end_exclusive: u64,
    /// Source byte range when the provider knows it exactly.
    pub source_range: Option<SourceRange>,
    /// Explicit domain units and coordinate, frame or sample convention.
    pub convention: String,
}

/// One verified typed artifact and its interpretation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct ChunkReference {
    /// Actual artifact bytes and integrity.
    pub artifact: Artifact,
    /// Typed element metadata.
    pub layout: ChunkLayout,
}

impl ChunkLayout {
    /// Describes an entire zero-based source-ordered array with explicit units or conventions.
    #[must_use]
    pub fn new(
        dtype: DataType,
        byte_order: ByteOrder,
        count: u64,
        convention: impl Into<String>,
    ) -> Self {
        Self {
            dtype,
            byte_order,
            order: DataOrder::Source,
            count,
            first: 0,
            end_exclusive: count,
            source_range: None,
            convention: convention.into(),
        }
    }
}

impl SourceRange {
    /// Describes a half-open byte range in its explicit source-relative basis.
    #[must_use]
    pub const fn new(basis: OffsetBasis, start: u64, end_exclusive: u64) -> Self {
        Self {
            basis,
            start,
            end_exclusive,
        }
    }
}

#[derive(Clone, Copy)]
enum ChunkAccounting {
    Converted,
    RetainedMetadata,
}

impl MediaWriter<'_> {
    /// Streams one exact typed chunk without collecting the array in memory.
    ///
    /// # Errors
    /// Returns invalid layout, excess or truncated data, budget, cancellation or file failures.
    pub fn chunk(
        &mut self,
        layout: ChunkLayout,
        input: &mut dyn Read,
    ) -> Result<ChunkReference, MediaError> {
        self.write_chunk(layout, input, ChunkAccounting::Converted)
    }

    pub(crate) fn metadata_chunk(
        &mut self,
        layout: ChunkLayout,
        input: &mut dyn Read,
    ) -> Result<ChunkReference, MediaError> {
        self.write_chunk(layout, input, ChunkAccounting::RetainedMetadata)
    }

    fn write_chunk(
        &mut self,
        layout: ChunkLayout,
        input: &mut dyn Read,
        accounting: ChunkAccounting,
    ) -> Result<ChunkReference, MediaError> {
        self.checkpoint()?;
        let size = chunk_size(&layout, self.limits.chunk_bytes)?;
        self.reserve_output(size)?;
        let directory = self.root.join("chunks");
        let chunk_copy::ChunkFile {
            mut temporary,
            digest,
        } = chunk_copy::copy(self, &directory, input, size)?;
        let artifact = Artifact {
            path: format!("chunks/{}", digest.hex),
            size,
            digest,
        };
        let result = (|| {
            let created = artifact_placement::place(
                self,
                &artifact,
                &mut temporary,
                "typed chunk content-address collision",
            )?;
            if matches!(accounting, ChunkAccounting::RetainedMetadata)
                && let Some(input) = self.origins.directory().cloned()
            {
                self.artifact_with_input(&artifact, &input, p4kx_extraction::InputScope::Payload)?;
            } else {
                self.artifact(&artifact)?;
            }
            if matches!(accounting, ChunkAccounting::Converted) {
                self.converted_output(created)?;
            }
            Ok(ChunkReference { artifact, layout })
        })();
        temporary.finish(result)
    }
}

fn chunk_size(layout: &ChunkLayout, limit: u64) -> Result<u64, MediaError> {
    if layout.end_exclusive.checked_sub(layout.first) != Some(layout.count)
        || layout.convention.is_empty()
        || layout
            .source_range
            .as_ref()
            .is_some_and(|range| range.end_exclusive < range.start)
    {
        return Err(MediaError::Contract(
            "invalid typed chunk range or missing domain convention".into(),
        ));
    }
    let size = layout
        .count
        .checked_mul(layout.dtype.width())
        .ok_or(MediaError::Limit {
            budget: "chunk_bytes",
            limit,
        })?;
    if size > limit {
        return Err(MediaError::Limit {
            budget: "chunk_bytes",
            limit,
        });
    }
    Ok(size)
}
