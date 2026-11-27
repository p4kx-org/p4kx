// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::{ByteOrder, Chunk};

/// Parsed physical table fields of one actually read model file's chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ModelChunkLayout {
    /// The layout version without the `CrCh` byte-order flag.
    pub version: u32,
    /// The declared chunk ID, absent in `#ivo`.
    pub id: Option<u32>,
    /// The chunk data's byte order without transforming the input bytes.
    pub byte_order: ByteOrder,
    /// The declared chunk-data offset within its owning decoded file.
    pub offset: u64,
    /// The byte span, declared in `CrCh` and inferred including padding in `#ivo`.
    pub size: u64,
}

impl ModelChunkLayout {
    pub(crate) const fn of(chunk: &Chunk) -> Self {
        Self {
            version: chunk.version(),
            id: chunk.id(),
            byte_order: chunk.byte_order(),
            offset: chunk.offset(),
            size: chunk.size(),
        }
    }
}
