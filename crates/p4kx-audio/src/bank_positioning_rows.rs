// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;
use std::slice;

use crate::bank_object_fields::BankHircField;
use crate::hirc_node_positioning::{BankPositioningTableKind, PATH_WORD_LEN, VERTEX_LEN};
use crate::read_at::SeekSource;
use crate::{BankError, BankReadError, SectionId};

/// Independently readable original coordinates in a positioning row.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankPositioningVector {
    /// Original binary32 X word.
    pub x_bits: BankHircField<u32>,
    /// Original binary32 Y word when complete.
    pub y_bits: Option<BankHircField<u32>>,
    /// Original binary32 Z word when complete.
    pub z_bits: Option<BankHircField<u32>>,
}

/// Original scalar fields of one path table row.
#[derive(Debug)]
#[non_exhaustive]
pub enum BankPositioningRowFields {
    /// Original path vertex.
    Vertex {
        /// Coordinate words.
        coordinates: BankPositioningVector,
        /// Original signed duration when complete.
        duration: Option<BankHircField<i32>>,
    },
    /// Original playlist occurrence.
    Playlist {
        /// Original vertex offset word.
        vertices_offset: BankHircField<u32>,
        /// Original vertex count when complete.
        vertex_count: Option<BankHircField<u32>>,
    },
    /// Original per-playlist-item automation range words.
    AutomationRange(BankPositioningVector),
}

/// Checked path table occurrence in physical order.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankPositioningRow {
    /// Zero-based physical row ordinal.
    pub ordinal: u32,
    /// Checked original row bytes, including an incomplete final word.
    pub checked_range: Range<u64>,
    /// Whether every authored field in the row fits.
    pub complete: bool,
    /// Checked scalar observations for the selected table.
    pub fields: BankPositioningRowFields,
}

/// Sequential cursor that replays checked rows from their owning soundbank.
pub struct BankPositioningRows<'input, R> {
    input: &'input mut SeekSource<R>,
    kind: BankPositioningTableKind,
    remaining: Range<u64>,
    ordinal: u32,
}

impl<'input, R> BankPositioningRows<'input, R> {
    pub(crate) const fn new(
        input: &'input mut SeekSource<R>,
        kind: BankPositioningTableKind,
        remaining: Range<u64>,
    ) -> Self {
        Self {
            input,
            kind,
            remaining,
            ordinal: 0,
        }
    }
}

impl<R: Read + Seek> Iterator for BankPositioningRows<'_, R> {
    type Item = Result<BankPositioningRow, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.end - self.remaining.start < PATH_WORD_LEN as u64 {
            return None;
        }
        let result = self.read();
        if result.is_err() {
            self.remaining.start = self.remaining.end;
        }
        Some(result)
    }
}

impl<R: Read + Seek> BankPositioningRows<'_, R> {
    fn read(&mut self) -> Result<BankPositioningRow, BankReadError> {
        let start = self.remaining.start;
        let width = self.kind.row_len() as u64;
        let len = (self.remaining.end - start).min(width);
        let complete_bytes = len / PATH_WORD_LEN as u64 * PATH_WORD_LEN as u64;
        let mut bytes = [0; VERTEX_LEN];
        let output = usize::try_from(complete_bytes)
            .ok()
            .and_then(|len| bytes.get_mut(..len))
            .ok_or_else(|| {
                BankError::invalid(
                    SectionId::HIRC,
                    "positioning row exceeds its checked buffer",
                )
            })?;
        self.input.read_io_at(start, output)?;
        let mut words = Words {
            chunks: output.as_chunks::<PATH_WORD_LEN>().0.iter(),
            offset: start,
        };
        let first = words.read(u32::from_le_bytes).ok_or_else(|| {
            BankError::invalid(
                SectionId::HIRC,
                "positioning row has no complete first word",
            )
        })?;
        let fields = match self.kind {
            BankPositioningTableKind::Vertex => BankPositioningRowFields::Vertex {
                coordinates: words.vector(first),
                duration: words.read(i32::from_le_bytes),
            },
            BankPositioningTableKind::Playlist => BankPositioningRowFields::Playlist {
                vertices_offset: first,
                vertex_count: words.read(u32::from_le_bytes),
            },
            BankPositioningTableKind::AutomationRange => {
                BankPositioningRowFields::AutomationRange(words.vector(first))
            }
        };
        let result = BankPositioningRow {
            ordinal: self.ordinal,
            checked_range: start..start + len,
            complete: len == width,
            fields,
        };
        self.remaining.start += len;
        self.ordinal += 1;
        Ok(result)
    }
}

struct Words<'bytes> {
    chunks: slice::Iter<'bytes, [u8; PATH_WORD_LEN]>,
    offset: u64,
}

impl Words<'_> {
    fn read<T>(
        &mut self,
        decode: impl FnOnce([u8; PATH_WORD_LEN]) -> T,
    ) -> Option<BankHircField<T>> {
        let bytes = *self.chunks.next()?;
        let start = self.offset;
        self.offset += PATH_WORD_LEN as u64;
        Some(BankHircField {
            value: decode(bytes),
            range: start..self.offset,
        })
    }

    fn vector(&mut self, x_bits: BankHircField<u32>) -> BankPositioningVector {
        BankPositioningVector {
            x_bits,
            y_bits: self.read(u32::from_le_bytes),
            z_bits: self.read(u32::from_le_bytes),
        }
    }
}
