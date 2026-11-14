// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use super::directional_paths::{DirectionalPath, PATH_LENGTH, path_text};

const WORD: usize = size_of::<u32>();
const PATH_AT: usize = 2 * WORD;
const CRC_AT: usize = PATH_AT + PATH_LENGTH;
const TAIL_AT: usize = CRC_AT + WORD;

#[derive(Clone, Copy)]
pub(super) struct DirectionalRecords<'a> {
    bytes: &'a [u8],
    start: u64,
    count: usize,
}

pub(super) struct DirectionalRecord<'a> {
    pub(super) range: Range<u64>,
    pub(super) byte_length: u32,
    pub(super) header_word: u32,
    pub(super) path_field: Range<u64>,
    pub(super) aim_path_crc32: u32,
    pub(super) opaque_tail: Range<u64>,
    pub(super) path: Option<DirectionalPath<'a>>,
}

impl<'a> DirectionalRecords<'a> {
    pub(super) fn new(bytes: &'a [u8], start: u64) -> Option<Self> {
        start.checked_add(bytes.len() as u64)?;
        let count = u32::from_le_bytes(*bytes.first_chunk::<WORD>()?) as usize;
        let records = bytes.get(WORD..)?;
        if count > records.len() / TAIL_AT {
            return None;
        }
        Some(Self {
            bytes: records,
            start: start + WORD as u64,
            count,
        })
    }

    pub(super) const fn count(self) -> usize {
        self.count
    }

    pub(super) const fn header(self) -> Range<u64> {
        self.start - WORD as u64..self.start
    }

    pub(super) fn visit<E>(
        mut self,
        mut visit: impl FnMut(usize, DirectionalRecord<'a>) -> Result<(), E>,
    ) -> Result<bool, E> {
        for ordinal in 0..self.count {
            let Some(record) = self.next_record() else {
                return Ok(false);
            };
            visit(ordinal, record)?;
        }
        Ok(self.bytes.is_empty())
    }

    fn next_record(&mut self) -> Option<DirectionalRecord<'a>> {
        let byte_length = word(self.bytes, 0)?;
        let length = byte_length as usize;
        if length < TAIL_AT || !length.is_multiple_of(WORD) {
            return None;
        }
        let (record, remaining) = self.bytes.split_at_checked(length)?;
        let start = self.start;
        let end = start + length as u64;
        let path_field = start + PATH_AT as u64..start + CRC_AT as u64;
        let original: &[u8; PATH_LENGTH] = record.get(PATH_AT..CRC_AT)?.try_into().ok()?;
        let path = path_text(original).map(|text| DirectionalPath {
            text,
            range: path_field.clone(),
        });
        self.bytes = remaining;
        self.start = end;
        Some(DirectionalRecord {
            range: start..end,
            byte_length,
            header_word: word(record, WORD)?,
            path_field,
            aim_path_crc32: word(record, CRC_AT)?,
            opaque_tail: start + TAIL_AT as u64..end,
            path,
        })
    }
}

fn word(bytes: &[u8], start: usize) -> Option<u32> {
    Some(u32::from_le_bytes(*bytes.get(start..)?.first_chunk()?))
}
