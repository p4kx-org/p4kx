// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::ModelFile;

/// The physical IVO source of a decoded mesh's vertex order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct MeshOrigin {
    /// The original file role resolved by the model read.
    pub file: ModelFile,
    /// The ordinal in the original file's chunk table.
    pub chunk: usize,
    /// The source geometry level before any fallback relabelling.
    pub lod: u32,
}

impl MeshOrigin {
    pub(crate) const fn new(file: ModelFile, chunk: usize, lod: u32) -> Self {
        Self { file, chunk, lod }
    }
}
