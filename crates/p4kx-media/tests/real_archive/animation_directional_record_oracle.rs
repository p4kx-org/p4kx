// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_media::{Identity, MediaRecord, Status};

use super::{
    CAF_CHUNK, CAF_IMAGE, CAF_RECORD_LENGTH, DIRECTIONAL_CHUNK, DIRECTIONAL_IMAGE, ImageSource,
    PATH_LENGTH, Scope, TestResult, compare_path, number, require, verify, verify_range,
};

#[path = "animation_directional_pose_oracle.rs"]
mod poses;

const COUNT_BYTES: usize = 4;
const HEADER_WORD_OFFSET: usize = 4;
const PATH_OFFSET: usize = 8;
const PATH_HASH_OFFSET: usize = PATH_OFFSET + PATH_LENGTH;
const FIXED_BYTES: usize = PATH_HASH_OFFSET + 4;

pub(super) struct Records<'a> {
    image: &'a ImageSource,
    records: Vec<Record>,
}

struct Record {
    range: Range<usize>,
    length: u32,
    header_word: u32,
    path_hash: u32,
    poses: Option<poses::Expected>,
}

impl Record {
    fn read(image: &ImageSource, start: usize, chunk_end: usize) -> TestResult<Self> {
        let remaining = image
            .bytes
            .get(start..chunk_end)
            .ok_or("directional record starts inside its original chunk")?;
        let length = word(remaining, 0)?;
        let relative_end = usize::try_from(length)?;
        require(
            relative_end >= FIXED_BYTES && relative_end.is_multiple_of(4),
            "directional record contains its fixed fields and remains word aligned",
        )?;
        let record = remaining
            .get(..relative_end)
            .ok_or("record extends beyond the original chunk")?;
        let field = record
            .get(PATH_OFFSET..PATH_HASH_OFFSET)
            .ok_or("original path field")?;
        let nul = field
            .iter()
            .position(|byte| *byte == 0)
            .ok_or("original AIM path terminator")?;
        let path = field.get(..nul).ok_or("original AIM path")?;
        require(
            !path.is_empty()
                && field
                    .get(nul..)
                    .is_some_and(|padding| padding.iter().all(|byte| *byte == 0)),
            "original AIM path field is nonempty and zero-padded",
        )?;
        let path_hash = word(record, PATH_HASH_OFFSET)?;
        require(
            path_hash == ieee_crc32(path),
            "the original path CRC-32 matches the ASCII-lowercased path",
        )?;
        Ok(Self {
            range: start..start + relative_end,
            length,
            header_word: word(record, HEADER_WORD_OFFSET)?,
            path_hash,
            poses: poses::Expected::read(record, start),
        })
    }

    const fn path_range(&self) -> Range<usize> {
        self.range.start + PATH_OFFSET..self.range.start + PATH_HASH_OFFSET
    }

    fn verify_row(&self, row: &MediaRecord) -> TestResult {
        let path = self.path_range();
        verify_range(row, "/payload/range", self.range.start, self.range.end)?;
        verify_range(row, "/payload/path_field", path.start, path.end)?;
        verify_range(
            row,
            "/payload/opaque_tail",
            self.range.start + FIXED_BYTES,
            self.range.end,
        )?;
        require(
            number(row, "/payload/byte_length")? == self.length as usize
                && number(row, "/payload/header_word")? == self.header_word as usize
                && number(row, "/payload/aim_path_crc32")? == self.path_hash as usize,
            "directional length, original header word and path CRC-32 match the physical record",
        )
    }
}

impl<'a> Records<'a> {
    pub(super) fn read(image: &'a ImageSource) -> TestResult<Self> {
        let chunk = &image.chunk;
        let mut cursor = chunk.start.checked_add(COUNT_BYTES).ok_or("header range")?;
        require(
            cursor <= chunk.end,
            "directional count word fits in its chunk",
        )?;
        let mut records = Vec::new();
        for _ in 0..image.count {
            let entry = Record::read(image, cursor, chunk.end)?;
            cursor = entry.range.end;
            records.push(entry);
        }
        require(
            cursor == chunk.end,
            "length-framed directional records end exactly at the original chunk end",
        )?;
        Ok(Self { image, records })
    }

    pub(super) fn require_poses(&self) -> TestResult {
        require(
            !self.records.is_empty() && self.records.iter().all(|record| record.poses.is_some()),
            "installed original records have fully framed pose inventories",
        )?;
        require(
            self.records.iter().any(|record| {
                record
                    .poses
                    .as_ref()
                    .is_some_and(poses::Expected::has_blocks)
            }),
            "installed original contains at least one pose block",
        )
    }

    pub(super) const fn replay(&self) -> Replay<'_, 'a> {
        Replay {
            expected: self,
            paths: 0,
            records: 0,
            retained: None,
            poses: poses::Observed::new(),
        }
    }
}

pub(super) struct Replay<'a, 'source> {
    expected: &'a Records<'source>,
    paths: usize,
    records: usize,
    retained: Option<Vec<u8>>,
    poses: poses::Observed,
}

impl Replay<'_, '_> {
    pub(super) fn index(&self, row: &MediaRecord) -> TestResult {
        require(
            entry_matches(row),
            "directional index uses the original Entry",
        )?;
        let start = self.expected.image.chunk.start;
        verify_range(row, "/payload/layout/header", start, start + COUNT_BYTES)?;
        require(
            number(row, "/payload/layout/record_count")? == self.expected.records.len(),
            "projected index retains its exact directional count",
        )
    }

    pub(super) fn artifact(&mut self, retained: Vec<u8>) -> TestResult {
        require(
            self.retained.replace(retained).is_none(),
            "each original directional image has one retained raw artifact",
        )
    }

    pub(super) fn path(&mut self, row: &MediaRecord) -> TestResult {
        let expected = self.expected_row(row, self.paths)?;
        let start = compare_path(row, &self.expected.image.bytes)?;
        let path = expected.path_range();
        verify_range(row, "/payload/path_field", path.start, path.end)?;
        require(
            start == path.start,
            "the compatibility path row uses the original framed AIM path",
        )?;
        self.paths += 1;
        Ok(())
    }

    pub(super) fn record(&mut self, row: &MediaRecord) -> TestResult {
        let expected = self.expected_row(row, self.records)?;
        require(
            row.data
                .get("record_type")
                .and_then(serde_json::Value::as_str)
                == Some("image-directional-record"),
            "published record type matches its namespace",
        )?;
        expected.verify_row(row)?;
        self.records += 1;
        Ok(())
    }

    pub(super) fn pose_index(&mut self, row: &MediaRecord) -> TestResult {
        let ordinal = Self::pose_owner(row)?;
        let index = self.expected.image.chunk.index as u64;
        self.poses.index(
            row,
            index,
            ordinal,
            expected_pose(self.expected.records.as_slice(), ordinal)?,
        )
    }

    pub(super) fn pose_block(&mut self, row: &MediaRecord) -> TestResult {
        let ordinal = Self::pose_owner(row)?;
        let index = self.expected.image.chunk.index as u64;
        self.poses.block(
            row,
            index,
            ordinal,
            expected_pose(self.expected.records.as_slice(), ordinal)?,
        )
    }

    pub(super) fn finish(self) -> TestResult {
        let retained = self.retained.ok_or("directional raw artifact is missing")?;
        let records = &self.expected.records;
        require(
            self.paths == records.len() && self.records == records.len(),
            "one projected path and one framed record for every original ordinal",
        )?;
        for (ordinal, record) in records.iter().enumerate() {
            self.poses.verify(ordinal, record.poses.as_ref())?;
            let range = record.range.clone();
            require(
                retained.get(range.clone()) == self.expected.image.bytes.get(range),
                "the complete framed record and its opaque tail survive byte-for-byte",
            )?;
        }
        Ok(())
    }

    fn pose_owner(row: &MediaRecord) -> TestResult<usize> {
        require(
            entry_matches(row),
            "pose row retains the original directional Entry",
        )?;
        let ordinal = row
            .subresource
            .indices
            .get(1)
            .ok_or("pose member's record ordinal")?;
        Ok(usize::try_from(*ordinal)?)
    }

    fn expected_row(&self, row: &MediaRecord, ordinal: usize) -> TestResult<&Record> {
        let chunk = self.expected.image.chunk.index as u64;
        require(
            row.status == Status::Complete
                && entry_matches(row)
                && row.subresource.indices.as_slice() == [chunk, ordinal as u64],
            "directional row is complete and keeps the original chunk and record ordinals",
        )?;
        self.expected
            .records
            .get(ordinal)
            .ok_or_else(|| "unexpected directional record ordinal".into())
    }
}

fn expected_pose(records: &[Record], ordinal: usize) -> TestResult<&poses::Expected> {
    records
        .get(ordinal)
        .and_then(|record| record.poses.as_ref())
        .ok_or_else(|| "pose projection requires complete physical framing".into())
}

fn entry_matches(row: &MediaRecord) -> bool {
    matches!(&row.source.identity, Identity::Entry { containers, entry }
        if containers.is_empty()
            && entry.path.replace('\\', "/").eq_ignore_ascii_case(DIRECTIONAL_IMAGE))
}

fn word(bytes: &[u8], offset: usize) -> TestResult<u32> {
    let field = bytes
        .get(offset..offset + 4)
        .ok_or("directional record word")?;
    Ok(u32::from_le_bytes(field.try_into()?))
}

fn ieee_crc32(bytes: &[u8]) -> u32 {
    const REVERSED_IEEE_POLYNOMIAL: u32 = 0xedb8_8320;
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(byte.to_ascii_lowercase());
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (REVERSED_IEEE_POLYNOMIAL & mask);
        }
    }
    !crc
}

pub(super) fn fixture() -> TestResult<Vec<u8>> {
    let mut body = Vec::from(3_u32.to_le_bytes());
    let counted = poses::fixture_tail(FIXED_BYTES);
    for (path, header, tail) in [
        (
            b"Animations/DEMO.AIM".as_slice(),
            0x1234_5678_u32,
            b"tail".as_slice(),
        ),
        (
            b"animations/another.aim".as_slice(),
            0x0000_0001_u32,
            b"opaque record bytes!".as_slice(),
        ),
        (b"Animations/counted.aim".as_slice(), 0, counted.as_slice()),
    ] {
        let length = u32::try_from(FIXED_BYTES + tail.len())?;
        body.extend(length.to_le_bytes());
        body.extend(header.to_le_bytes());
        body.extend(path);
        body.resize(body.len() + PATH_LENGTH - path.len(), 0);
        body.extend(crc32fast::hash(&path.to_ascii_lowercase()).to_le_bytes());
        body.extend(tail);
    }
    Ok(body)
}

#[test]
fn the_independent_path_crc_matches_the_ieee_known_answer() {
    assert_eq!(
        ieee_crc32(b"123456789"),
        0xcbf4_3926,
        "standard reflected IEEE CRC-32 check vector",
    );
}

#[test]
fn animation_image_replay_checks_raw_bytes_headers_and_exact_trailer() -> TestResult {
    use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestEntry};

    let mut caf = Vec::new();
    caf.extend(1_u32.to_le_bytes());
    caf.extend(2_u32.to_le_bytes());
    let mut record = [0; CAF_RECORD_LENGTH];
    let path = b"animations/demo.caf";
    record[..path.len()].copy_from_slice(path);
    record[256..260].copy_from_slice(&0x1234_5678_u32.to_le_bytes());
    caf.extend(record);
    caf.extend(*b"trailer!");
    let caf = TestChunkFile::ivo()
        .chunk(0x1234, 0x900, [9, 8])
        .chunk(CAF_CHUNK, 0x902, caf)
        .bytes();
    let directional = TestChunkFile::ivo()
        .chunk(0x1234, 0x900, [7, 6, 5])
        .chunk(DIRECTIONAL_CHUNK, 0x901, fixture()?)
        .bytes();
    let folder = tempfile::tempdir()?;
    let input = folder.path().join("fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(CAF_IMAGE, caf))
        .entry(TestEntry::stored(DIRECTIONAL_IMAGE, directional))
        .entry(TestEntry::stored("Data/unrelated.pak", b"not an archive"))
        .write_to(&input)?;
    verify(&input, Scope::Both)
}
