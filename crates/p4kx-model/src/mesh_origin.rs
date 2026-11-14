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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MeshSubmeshOrigin {
    pub(crate) start: u64,
    pub(crate) index_start: u64,
    pub(crate) page_base: u32,
}

impl MeshSubmeshOrigin {
    pub(crate) const fn new(start: u64, index_start: u64, page_base: u32) -> Self {
        Self {
            start,
            index_start,
            page_base,
        }
    }
}

/// The first original index word and page selector used for an output vertex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct MeshVertexOrigin {
    /// The output POSITION ordinal.
    pub output_vertex: u32,
    /// The source-stream vertex ordinal after adding the page base.
    pub source_vertex: u32,
    /// The original little-endian word before adding the page base.
    pub index_word: u32,
    /// The original source page base.
    pub page_base: u32,
    index_start: u64,
    index_width: u32,
    submesh_start: u64,
}

impl MeshVertexOrigin {
    /// Returns the original index word's half-open range within the mesh chunk.
    #[must_use]
    pub const fn index_range(self) -> std::ops::Range<u64> {
        self.index_start..self.index_start + self.index_width as u64
    }

    /// Returns the original first-index and index-count words within the mesh chunk.
    #[must_use]
    pub const fn index_selection_range(self) -> std::ops::Range<u64> {
        self.submesh_start + 4..self.submesh_start + 12
    }

    /// Returns the original page-base word within the mesh chunk.
    #[must_use]
    pub const fn page_base_range(self) -> std::ops::Range<u64> {
        self.submesh_start + 16..self.submesh_start + 20
    }
}

impl crate::Mesh {
    /// Returns whether the decoded ordering retains its original first-use index coordinates.
    #[must_use]
    pub const fn has_vertex_origin_order(&self) -> bool {
        self.origin.is_some() && self.index_width.is_some() && !self.original_vertex_order_changed
    }

    /// Clears first-use coordinates when the authored index traversal changes.
    pub const fn invalidate_vertex_origin_order(&mut self) {
        self.original_vertex_order_changed = true;
    }

    /// Returns first-use coordinates in emitted vertex order for a decoded IVO mesh.
    pub fn vertex_origins(&self) -> impl Iterator<Item = MeshVertexOrigin> + '_ {
        let mut next = 0_u32;
        self.submeshes
            .iter()
            .flat_map(move |submesh| {
                let source = self
                    .index_width
                    .zip(submesh.origin)
                    .filter(|_| !self.original_vertex_order_changed);
                let indices = self.submesh_indices(submesh).unwrap_or_default();
                indices
                    .iter()
                    .enumerate()
                    .filter_map(move |(position, &dense)| {
                        let (width, source) = source?;
                        let source_vertex = *self.source.get(dense as usize)?;
                        Some(MeshVertexOrigin {
                            output_vertex: dense,
                            source_vertex,
                            index_word: source_vertex.checked_sub(source.page_base)?,
                            page_base: source.page_base,
                            index_start: source.index_start + position as u64 * u64::from(width),
                            index_width: width,
                            submesh_start: source.start,
                        })
                    })
            })
            .filter(move |reference| {
                // Decoder indices encounter each dense vertex in order at its first occurrence.
                if reference.output_vertex != next {
                    return false;
                }
                next += 1;
                true
            })
    }
}
