// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_foundation::ByteReader;

use crate::physics_blob::{PhysicsBlobError, PhysicsFloat, read_vector};

const WORD: usize = size_of::<u32>();
const HALF_WORD: usize = size_of::<u16>();
const VERTEX_BYTES: usize = 3 * WORD;
const TRIANGLE_BYTES: usize = 3 * HALF_WORD;

/// Original positioned arrays of one validated collision triangle mesh.
#[derive(Debug, Clone)]
pub struct PhysicsTrimesh<'source> {
    count_offset: u64,
    material_ids: &'source [u8],
    material_offset: u64,
    vertex_words: &'source [u8],
    vertex_offset: u64,
    index_words: &'source [u8],
    index_offset: u64,
}

impl<'source> PhysicsTrimesh<'source> {
    pub(crate) fn read(reader: &mut ByteReader<'source>) -> Result<Self, PhysicsBlobError> {
        let count_offset = reader.offset();
        let vertex_count = count(reader)?;
        let triangle_count = count(reader)?;
        reader.skip(2 * WORD)?;
        let leading = count(reader)?;
        let mapped = reader.read_bytes(2)?.first().is_some_and(|&word| word != 0);
        skip_items(reader, leading, HALF_WORD)?;
        if mapped {
            skip_items(reader, triangle_count, HALF_WORD)?;
        }
        let material_offset = reader.offset();
        let material_ids = reader.read_bytes(triangle_count)?;
        let vertex_offset = reader.offset();
        let vertex_words = items(reader, vertex_count, VERTEX_BYTES)?;
        let index_offset = reader.offset();
        let index_words = items(reader, triangle_count, TRIANGLE_BYTES)?;

        let mut vertices = ByteReader::with_base(vertex_words, vertex_offset);
        for _ in 0..vertex_count {
            read_vector(&mut vertices)?;
        }
        let mut indices = ByteReader::with_base(index_words, index_offset);
        for _ in 0..index_words.len() / HALF_WORD {
            let offset = indices.offset();
            let index = indices.read_u16()?;
            if usize::from(index) >= vertex_count {
                return Err(PhysicsBlobError::Index {
                    offset,
                    index,
                    vertices: vertex_count,
                });
            }
        }
        Ok(Self {
            count_offset,
            material_ids,
            material_offset,
            vertex_words,
            vertex_offset,
            index_words,
            index_offset,
        })
    }

    /// Returns the original vertex count word's range relative to the blob.
    #[must_use]
    pub const fn vertex_count_range(&self) -> Range<u64> {
        self.count_offset..self.count_offset + WORD as u64
    }

    /// Returns the original triangle count word's range relative to the blob.
    #[must_use]
    pub const fn triangle_count_range(&self) -> Range<u64> {
        self.count_offset + WORD as u64..self.count_offset + 2 * WORD as u64
    }

    /// Returns the original material bytes in triangle order.
    #[must_use]
    pub const fn material_ids(&self) -> &'source [u8] {
        self.material_ids
    }

    /// Returns the original range of material bytes relative to the blob.
    #[must_use]
    pub const fn material_range(&self) -> Range<u64> {
        self.material_offset..self.material_offset + self.material_ids.len() as u64
    }

    /// Returns the original range of source vertices relative to the blob.
    #[must_use]
    pub const fn vertex_range(&self) -> Range<u64> {
        self.vertex_offset..self.vertex_offset + self.vertex_words.len() as u64
    }

    /// Returns the original range of triangle indices relative to the blob.
    #[must_use]
    pub const fn index_range(&self) -> Range<u64> {
        self.index_offset..self.index_offset + self.index_words.len() as u64
    }

    /// Iterates checked finite vertices in source order, with original word positions.
    pub fn vertices(&self) -> impl ExactSizeIterator<Item = [PhysicsFloat; 3]> + '_ {
        self.vertex_words
            .as_chunks::<VERTEX_BYTES>()
            .0
            .iter()
            .enumerate()
            .map(|(ordinal, words)| {
                let &[a0, a1, a2, a3, b0, b1, b2, b3, c0, c1, c2, c3] = words;
                let components = [[a0, a1, a2, a3], [b0, b1, b2, b3], [c0, c1, c2, c3]];
                let start = self.vertex_offset + ordinal as u64 * VERTEX_BYTES as u64;
                let mut offset = start;
                components.map(|component| {
                    let value =
                        PhysicsFloat::from_validated_bits(u32::from_le_bytes(component), offset);
                    offset += WORD as u64;
                    value
                })
            })
    }

    /// Iterates checked triangle index triplets in source order.
    pub fn triangles(&self) -> impl ExactSizeIterator<Item = [u16; 3]> + '_ {
        self.index_words
            .as_chunks::<TRIANGLE_BYTES>()
            .0
            .iter()
            .map(|words| {
                let &[a0, a1, b0, b1, c0, c1] = words;
                [
                    u16::from_le_bytes([a0, a1]),
                    u16::from_le_bytes([b0, b1]),
                    u16::from_le_bytes([c0, c1]),
                ]
            })
    }
}

fn count(reader: &mut ByteReader<'_>) -> Result<usize, PhysicsBlobError> {
    let offset = reader.offset();
    let word = reader.read_u32()?;
    let signed = i32::from_le_bytes(word.to_le_bytes());
    usize::try_from(signed).map_err(|_| PhysicsBlobError::Count { offset })
}

fn items<'source>(
    reader: &mut ByteReader<'source>,
    count: usize,
    size: usize,
) -> Result<&'source [u8], PhysicsBlobError> {
    let bytes = count
        .checked_mul(size)
        .ok_or_else(|| PhysicsBlobError::Count {
            offset: reader.offset(),
        })?;
    Ok(reader.read_bytes(bytes)?)
}

fn skip_items(
    reader: &mut ByteReader<'_>,
    count: usize,
    size: usize,
) -> Result<(), PhysicsBlobError> {
    items(reader, count, size).map(|_| ())
}
