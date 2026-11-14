// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_chunks::ChunkTableRow;

#[derive(Debug, Clone, Copy)]
pub(crate) enum MatrixRead {
    Identity,
    Rows,
    Transposed,
}

/// Original locations of one admitted node property.
#[derive(Debug)]
pub struct ModelNodePropertySource {
    pub(crate) line: Range<u64>,
    pub(crate) key: Range<u64>,
    pub(crate) value: Range<u64>,
}

impl ModelNodePropertySource {
    /// Returns the original line tested for a property.
    #[must_use]
    pub fn line(&self) -> Range<u64> {
        self.line.clone()
    }

    /// Returns the original trimmed key bytes.
    #[must_use]
    pub fn key(&self) -> Range<u64> {
        self.key.clone()
    }

    /// Returns the original trimmed value bytes, which can be empty.
    #[must_use]
    pub fn value(&self) -> Range<u64> {
        self.value.clone()
    }
}

/// Original byte ranges used to decode one node of a model.
#[derive(Debug)]
pub struct ModelNodeSource {
    pub(crate) chunk: usize,
    pub(crate) name: Option<Range<u64>>,
    pub(crate) parent: Range<u64>,
    pub(crate) matrix: Range<u64>,
    pub(crate) table: Option<ChunkTableRow>,
    pub(crate) matrix_read: MatrixRead,
    pub(crate) scale: Option<Range<u64>>,
    pub(crate) scale_read: bool,
    pub(crate) geometry_type: Option<Range<u64>>,
    pub(crate) properties: Option<Range<u64>>,
    pub(crate) property_rows: Vec<ModelNodePropertySource>,
}

impl ModelNodeSource {
    /// Returns the physical chunk's table ordinal.
    #[must_use]
    pub const fn chunk(&self) -> usize {
        self.chunk
    }

    /// Returns the half-open name range in the primary file.
    #[must_use]
    pub fn name(&self) -> Option<Range<u64>> {
        self.name.clone()
    }

    /// Returns the half-open range of the original parent field.
    #[must_use]
    pub fn parent(&self) -> Range<u64> {
        self.parent.clone()
    }

    /// Returns the half-open range of the original local matrix.
    #[must_use]
    pub fn matrix(&self) -> Range<u64> {
        self.matrix.clone()
    }

    /// Returns the half-open range of the node's chunk ID, when stored.
    #[must_use]
    pub fn id(&self) -> Option<Range<u64>> {
        self.table.as_ref()?.id.clone()
    }

    /// Returns the original table row of a separately stored node chunk.
    #[must_use]
    pub const fn table_row(&self) -> Option<&ChunkTableRow> {
        self.table.as_ref()
    }

    /// Returns the original scalar supplying this output cell, if it was copied.
    #[must_use]
    pub const fn matrix_component(&self, row: usize, column: usize) -> Option<Range<u64>> {
        if row >= 3 || column >= 4 {
            return None;
        }
        let word = match self.matrix_read {
            MatrixRead::Identity => return None,
            MatrixRead::Rows => row * 4 + column,
            MatrixRead::Transposed => column * 4 + row,
        };
        Some(float_word(self.matrix.start, word))
    }

    /// Returns the three source words used to select the scale.
    #[must_use]
    pub fn scale(&self) -> Option<Range<u64>> {
        self.scale.clone()
    }

    /// Returns the copied scale word, when the scale did not use its default.
    #[must_use]
    pub fn scale_component(&self, axis: usize) -> Option<Range<u64>> {
        (self.scale_read && axis < 3)
            .then(|| {
                self.scale
                    .as_ref()
                    .map(|scale| float_word(scale.start, axis))
            })
            .flatten()
    }

    /// Returns the original geometry-type word, when the layout stores one.
    #[must_use]
    pub fn geometry_type(&self) -> Option<Range<u64>> {
        self.geometry_type.clone()
    }

    /// Returns the original property entry and its terminating separator.
    #[must_use]
    pub fn properties(&self) -> Option<Range<u64>> {
        self.properties.clone()
    }

    /// Returns the original property sites in retained order.
    #[must_use]
    pub fn property_rows(&self) -> &[ModelNodePropertySource] {
        &self.property_rows
    }
}

const fn float_word(start: u64, word: usize) -> Range<u64> {
    let width = size_of::<f32>() as u64;
    let at = start + word as u64 * width;
    at..at + width
}
