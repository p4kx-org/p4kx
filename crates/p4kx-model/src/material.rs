// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::{Chunk, ChunkType, crch, ivo};

const NAME_LEN: usize = 128;
const CRCH_MATERIAL_NAME_VERSION: u32 = 0x802;

/// A bounded material-library name declared by a model chunk.
pub struct MaterialName;

impl MaterialName {
    /// Reports which physical chunk types can carry a material-library name.
    #[must_use]
    pub const fn is_chunk_type(kind: ChunkType) -> bool {
        matches!(kind, ivo::MTL_NAME | ivo::MTL_NAME_320 | crch::MTL_NAME)
    }

    /// Reports whether the chunk uses a recognized name layout.
    #[must_use]
    pub const fn supports(chunk: &Chunk) -> bool {
        match chunk.chunk_type() {
            ivo::MTL_NAME | ivo::MTL_NAME_320 => true,
            crch::MTL_NAME => chunk.version() == CRCH_MATERIAL_NAME_VERSION,
            _ => false,
        }
    }

    /// Returns the original table ordinal selected by the model reader.
    #[must_use]
    pub fn primary(chunks: &[Chunk]) -> Option<usize> {
        [ivo::MTL_NAME_320, ivo::MTL_NAME, crch::MTL_NAME]
            .into_iter()
            .find_map(|kind| {
                chunks
                    .iter()
                    .position(|chunk| chunk.chunk_type() == kind && Self::supports(chunk))
            })
    }

    /// Returns the readable prefix length of the bounded name field.
    #[must_use]
    pub const fn prefix_len(chunk_len: u64) -> u64 {
        if chunk_len < NAME_LEN as u64 {
            chunk_len
        } else {
            NAME_LEN as u64
        }
    }

    /// Returns the original name bytes before the first NUL within the bounded field.
    #[must_use]
    pub fn bytes(data: &[u8]) -> &[u8] {
        let field = data.get(..NAME_LEN).unwrap_or(data);
        field.split(|&byte| byte == 0).next().unwrap_or(field)
    }
}

/// Returns the material library a `MtlName` chunk names: the text of its first 128 bytes up to
/// the first NUL, or `None` when that is empty.
pub(crate) fn material_library(data: &[u8]) -> Option<String> {
    let name = String::from_utf8_lossy(MaterialName::bytes(data));
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}
