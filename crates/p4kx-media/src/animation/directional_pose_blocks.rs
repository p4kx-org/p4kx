// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use super::directional_records::DirectionalRecord;

const COUNT_AT: usize = 2_156;
const WORD: usize = size_of::<u32>();
const POSE_HEADER_BYTES: usize = 3 * WORD;
const QUATERNION_BYTES: usize = 4 * size_of::<f32>();
const VECTOR_BYTES: usize = 3 * size_of::<f32>();
const EXTENSION_BYTES: usize = 552;

#[derive(Clone, Copy)]
pub(super) struct DirectionalPoseBlocks<'a> {
    tail: &'a [u8],
    start: u64,
    pub(super) pose_count: u32,
    pub(super) extension_count: u32,
    pub(super) header_word: u32,
}

pub(super) struct DirectionalPoseBlock {
    pub(super) entries4: PoseArray,
    pub(super) entries3: PoseArray,
}

pub(super) struct PoseArray {
    pub(super) count: u32,
    pub(super) count_range: Range<u64>,
    pub(super) values: Range<u64>,
}

impl<'a> DirectionalPoseBlocks<'a> {
    pub(super) fn new(record: &DirectionalRecord<'a>) -> Option<Self> {
        let path_at = usize::try_from(record.path_field.start - record.range.start).ok()?;
        let pose_header = path_at.checked_add(COUNT_AT)?;
        let original = record.bytes.get(pose_header..)?;
        let words = original
            .first_chunk::<POSE_HEADER_BYTES>()?
            .as_chunks::<WORD>()
            .0;
        let [pose_count, extension_count, header_word] = words else {
            return None;
        };
        let pose_count = u32::from_le_bytes(*pose_count);
        let extension_count = u32::from_le_bytes(*extension_count);
        let extension_bytes = (extension_count as usize).checked_mul(EXTENSION_BYTES)?;
        let body = original.get(POSE_HEADER_BYTES..)?;
        let pose_len = body.len().checked_sub(extension_bytes)?;
        let tail = body.get(..pose_len)?;
        if pose_count as usize > tail.len() / (2 * WORD) {
            return None;
        }
        Some(Self {
            tail,
            start: record.range.start + (pose_header + POSE_HEADER_BYTES) as u64,
            pose_count,
            extension_count,
            header_word: u32::from_le_bytes(*header_word),
        })
    }

    pub(super) const fn header(self) -> Range<u64> {
        self.start - POSE_HEADER_BYTES as u64..self.start
    }

    pub(super) const fn extensions(self) -> Range<u64> {
        let start = self.start + self.tail.len() as u64;
        start..start + (self.extension_count as u64) * EXTENSION_BYTES as u64
    }

    pub(super) fn visit<E>(
        mut self,
        mut visit: impl FnMut(u32, DirectionalPoseBlock) -> Result<(), E>,
    ) -> Result<bool, E> {
        for ordinal in 0..self.pose_count {
            let Some(entries4) = self.array(QUATERNION_BYTES) else {
                return Ok(false);
            };
            let Some(entries3) = self.array(VECTOR_BYTES) else {
                return Ok(false);
            };
            visit(ordinal, DirectionalPoseBlock { entries4, entries3 })?;
        }
        Ok(self.tail.is_empty())
    }

    fn array(&mut self, element_bytes: usize) -> Option<PoseArray> {
        let count = u32::from_le_bytes(*self.tail.first_chunk::<WORD>()?);
        let bytes = (count as usize).checked_mul(element_bytes)?;
        let end = WORD.checked_add(bytes)?;
        self.tail = self.tail.get(end..)?;
        let count_range = self.start..self.start + WORD as u64;
        let values = count_range.end..self.start + end as u64;
        self.start = values.end;
        Some(PoseArray {
            count,
            count_range,
            values,
        })
    }
}
