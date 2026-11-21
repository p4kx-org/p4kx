// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::ops::Range;

use p4kx_media::{MediaRecord, Status};

use super::{PATH_OFFSET, TestResult, number, require, verify_range};

const COUNT_AT: usize = PATH_OFFSET + 2156;
const FIRST_BLOCK_AT: usize = COUNT_AT + 3 * size_of::<u32>();
const EXTENSION_SIZE: usize = 552;

pub(super) struct Expected {
    header: Range<usize>,
    pose_count: usize,
    extension_count: usize,
    header_word: usize,
    extensions: Range<usize>,
    blocks: Vec<[LaneStream; 2]>,
}

struct LaneStream {
    count: usize,
    count_range: Range<usize>,
    values: Range<usize>,
}

impl Expected {
    pub(super) fn read(original: &[u8], absolute: usize) -> Option<Self> {
        Self::framing(original).map(|frame| frame.positioned(absolute))
    }

    fn framing(original: &[u8]) -> Option<Self> {
        let mut words = original
            .get(COUNT_AT..FIRST_BLOCK_AT)?
            .as_chunks::<4>()
            .0
            .iter();
        let pose_count = integer(words.next()?)?;
        let extension_count = integer(words.next()?)?;
        let header_word = integer(words.next()?)?;
        let extension_bytes = extension_count.checked_mul(EXTENSION_SIZE)?;
        let extensions = original.len().checked_sub(extension_bytes)?..original.len();
        let strokes = original.get(FIRST_BLOCK_AT..extensions.start)?;
        let mut cursor = FIRST_BLOCK_AT;
        if pose_count > strokes.len() / (2 * size_of::<u32>()) {
            return None;
        }
        let mut remaining = strokes;
        let mut blocks = Vec::new();
        for _ in 0..pose_count {
            let streams = [
                lane_stream(&mut remaining, &mut cursor, 16)?,
                lane_stream(&mut remaining, &mut cursor, 12)?,
            ];
            blocks.push(streams);
        }
        remaining.is_empty().then_some(Self {
            header: COUNT_AT..FIRST_BLOCK_AT,
            pose_count,
            extension_count,
            header_word,
            extensions,
            blocks,
        })
    }

    fn positioned(mut self, absolute: usize) -> Self {
        let position = |range: &mut Range<usize>| {
            range.start += absolute;
            range.end += absolute;
        };
        position(&mut self.header);
        position(&mut self.extensions);
        for item in self.blocks.iter_mut().flatten() {
            position(&mut item.count_range);
            position(&mut item.values);
        }
        self
    }

    pub(super) const fn has_blocks(&self) -> bool {
        !self.blocks.is_empty()
    }

    fn index(&self, row: &MediaRecord) -> TestResult {
        range(row, "/payload/header", &self.header)?;
        range(row, "/payload/extensions", &self.extensions)?;
        let path = self.header.start - 2156;
        for (field, start, end) in [
            ("first_orientation", 288, 304),
            ("second_orientation", 304, 320),
            ("polar_grid", 320, 2156),
        ] {
            verify_range(row, &format!("/payload/{field}"), path + start, path + end)?;
        }
        require(
            number(row, "/payload/pose_count")? == self.pose_count
                && number(row, "/payload/extension_count")? == self.extension_count
                && number(row, "/payload/header_word")? == self.header_word,
            "pose index retains original counts and the untyped word",
        )
    }

    fn block(&self, row: &MediaRecord, ordinal: usize) -> TestResult {
        let [entries4, entries3] = self.blocks.get(ordinal).ok_or("original pose ordinal")?;
        entries4.verify(row, "/payload/entries4")?;
        entries3.verify(row, "/payload/entries3")
    }
}

impl LaneStream {
    fn verify(&self, row: &MediaRecord, key: &str) -> TestResult {
        require(
            number(row, &format!("{key}/count"))? == self.count,
            "pose array has the original declared entry count",
        )?;
        range(row, &format!("{key}/count_range"), &self.count_range)?;
        range(row, &format!("{key}/values"), &self.values)
    }
}

pub(super) struct Observed {
    records: BTreeMap<usize, Coverage>,
}

#[derive(Default)]
struct Coverage {
    indexes: usize,
    blocks: usize,
}

impl Observed {
    pub(super) const fn new() -> Self {
        Self {
            records: BTreeMap::new(),
        }
    }

    pub(super) fn index(
        &mut self,
        row: &MediaRecord,
        chunk: u64,
        record: usize,
        expected: &Expected,
    ) -> TestResult {
        identity(row, "image-directional-pose-index", &[chunk, record as u64])?;
        expected.index(row)?;
        self.records.entry(record).or_default().indexes += 1;
        Ok(())
    }

    pub(super) fn block(
        &mut self,
        row: &MediaRecord,
        chunk: u64,
        record: usize,
        expected: &Expected,
    ) -> TestResult {
        let coverage = self.records.entry(record).or_default();
        identity(
            row,
            "image-directional-pose-block",
            &[chunk, record as u64, coverage.blocks as u64],
        )?;
        expected.block(row, coverage.blocks)?;
        coverage.blocks += 1;
        Ok(())
    }

    pub(super) fn verify(&self, ordinal: usize, expected: Option<&Expected>) -> TestResult {
        match (expected, self.records.get(&ordinal)) {
            (Some(expected), Some(observed)) => require(
                observed.indexes == 1 && observed.blocks == expected.blocks.len(),
                "all framed poses reconcile to exactly one index and every physical block",
            ),
            (None, None) => Ok(()),
            _ => Err("pose projection disagrees with complete physical framing".into()),
        }
    }
}

fn identity(row: &MediaRecord, record_type: &str, indices: &[u64]) -> TestResult {
    require(
        row.status == Status::Complete
            && row.subresource.indices.as_slice() == indices
            && row
                .data
                .get("record_type")
                .and_then(serde_json::Value::as_str)
                == Some(record_type),
        "pose row carries the expected type and original chunk, record and pose ordinals",
    )
}

fn range(row: &MediaRecord, key: &str, expected: &Range<usize>) -> TestResult {
    verify_range(row, key, expected.start, expected.end)
}

fn integer(bytes: &[u8]) -> Option<usize> {
    let value = u32::from_le_bytes(bytes.try_into().ok()?);
    usize::try_from(value).ok()
}

fn lane_stream(remaining: &mut &[u8], cursor: &mut usize, stride: usize) -> Option<LaneStream> {
    let (raw_count, values) = remaining.split_at_checked(4)?;
    let count = integer(raw_count)?;
    let bytes = count.checked_mul(stride)?;
    let (_, rest) = values.split_at_checked(bytes)?;
    let value_start = cursor.checked_add(4)?;
    let end = value_start.checked_add(bytes)?;
    let count_range = *cursor..value_start;
    *cursor = end;
    *remaining = rest;
    Some(LaneStream {
        count,
        count_range,
        values: value_start..end,
    })
}

pub(super) fn fixture_tail(fixed_prefix: usize) -> Vec<u8> {
    let mut bytes = vec![0; COUNT_AT - fixed_prefix];
    for word in [2_u32, 1, 0x4433_2211] {
        bytes.extend(word.to_le_bytes());
    }
    for (entries4, entries3) in [(2_u32, 0_u32), (1, 3)] {
        bytes.extend(entries4.to_le_bytes());
        bytes.extend(std::iter::repeat_n(0x65, entries4 as usize * 16));
        bytes.extend(entries3.to_le_bytes());
        bytes.extend(std::iter::repeat_n(0xa2, entries3 as usize * 12));
    }
    bytes.extend([0x39; EXTENSION_SIZE]);
    bytes
}

#[test]
fn pose_oracle_rejects_truncation_and_extra_bytes() {
    let bytes = fixture_tail(0);
    assert!(
        Expected::framing(&bytes).is_some(),
        "exact counted pose fixture"
    );
    assert!(
        bytes
            .split_last()
            .is_some_and(|(_, short)| Expected::framing(short).is_none()),
        "a short extension cannot consume bytes outside the record"
    );
    let mut trailing = bytes;
    trailing.push(0);
    assert!(
        Expected::framing(&trailing).is_none(),
        "an extra byte has no owner"
    );
}
