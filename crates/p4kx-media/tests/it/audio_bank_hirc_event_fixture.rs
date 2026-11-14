// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{HircValue, Soundbank};
use serde_json::{Value, json};

use super::TestResult;

pub(super) const ID: u32 = 0x4455_6677;
pub(super) const TAIL: [u8; 2] = [0xee, 0x99];
const EVENT_TAG: u8 = 4;
const SECTION_HEADER_BYTES: usize = 8;

pub(super) struct Fixture {
    pub(super) bytes: Vec<u8>,
    pub(super) version: u32,
    pub(super) count: Range<usize>,
    pub(super) actions: Vec<u32>,
    pub(super) trailing: Range<usize>,
}

pub(super) fn fixed() -> TestResult<Fixture> {
    fixture(122, &3_u32.to_le_bytes(), vec![42, 42, 91], &TAIL)
}

pub(super) fn variable() -> TestResult<Fixture> {
    let actions = (0..130).map(|index| 42 + index / 2).collect();
    fixture(123, &[0x81, 0x02], actions, &TAIL)
}

pub(super) fn malformed() -> TestResult<Fixture> {
    fixture(154, &[3], vec![42], &[])
}

pub(super) fn unsupported() -> TestResult<Fixture> {
    fixture(155, &[1], vec![42], &[])
}

fn fixture(version: u32, count: &[u8], actions: Vec<u32>, tail: &[u8]) -> TestResult<Fixture> {
    let mut bytes = Vec::new();
    let header = [version, 77, 0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    section(&mut bytes, *b"BKHD", &header)?;
    let base = bytes.len() + SECTION_HEADER_BYTES;
    let mut hirc = 1_u32.to_le_bytes().to_vec();
    hirc.push(EVENT_TAG);
    let length = size_of::<u32>() + count.len() + actions.len() * size_of::<u32>() + tail.len();
    hirc.extend_from_slice(&u32::try_from(length)?.to_le_bytes());
    hirc.extend_from_slice(&ID.to_le_bytes());
    let start = base + hirc.len();
    hirc.extend_from_slice(count);
    for action in &actions {
        hirc.extend_from_slice(&action.to_le_bytes());
    }
    let trailing = base + hirc.len()..base + hirc.len() + tail.len();
    hirc.extend_from_slice(tail);
    section(&mut bytes, *b"HIRC", &hirc)?;
    Ok(Fixture {
        bytes,
        version,
        count: start..start + count.len(),
        actions,
        trailing,
    })
}

impl Fixture {
    pub(super) fn check_legacy(&self) -> TestResult {
        let bank = Soundbank::parse(self.bytes.as_slice())?;
        let object = bank.objects().first().ok_or("original Event")?;
        let HircValue::Event(actions) = &object.value else {
            return Err("legacy Soundbank decoded the valid Event".into());
        };
        assert_eq!(
            actions, &self.actions,
            "legacy action IDs and duplicate order"
        );
        Ok(())
    }

    pub(super) const fn action_range(&self, ordinal: usize) -> Range<usize> {
        let offset = self.count.end + ordinal * size_of::<u32>();
        offset..offset + size_of::<u32>()
    }

    pub(super) const fn encoding(&self) -> &'static str {
        if self.version == 122 {
            "fixed-u32"
        } else {
            "variable-length"
        }
    }
}

pub(super) fn range(extent: &Range<usize>) -> Value {
    json!({"basis":{"type":"entry"},"start":extent.start.to_string(),"end_exclusive":extent.end.to_string()})
}

fn section(bytes: &mut Vec<u8>, tag: [u8; 4], body: &[u8]) -> TestResult {
    bytes.extend_from_slice(&tag);
    bytes.extend_from_slice(&u32::try_from(body.len())?.to_le_bytes());
    bytes.extend_from_slice(body);
    Ok(())
}
