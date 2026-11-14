// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::SNIFF_LEN;

use crate::layout_field_fixture::{self as fixture, BASE, DERIVED, TestResult};

const STRUCT_BYTES: usize = 16;
const PROPERTY_BYTES: usize = 12;
const MAPPING_BYTES: usize = 8;
const RECORD_BYTES: usize = 36;

pub(crate) struct Tables {
    pub(crate) structures: usize,
    pub(crate) properties: usize,
    pub(crate) mappings: usize,
    pub(crate) records: usize,
    pub(crate) text: usize,
    pub(crate) blob: usize,
    pub(crate) instances: usize,
}

impl Tables {
    pub(crate) fn read(bytes: &[u8]) -> TestResult<Self> {
        assert_eq!(
            (word(bytes, 0)?, word(bytes, 4)?),
            (0, 8),
            "the actual source header is version8"
        );
        assert_eq!(word(bytes, 24)?, 0, "the fixture has no enum table");
        let pools = bytes
            .get(36..SNIFF_LEN - 8)
            .ok_or("the actual pool counts")?;
        assert!(
            pools.iter().all(|byte| *byte == 0),
            "these scalar fixtures have no value pools"
        );
        let structures = SNIFF_LEN;
        let properties = end(structures, word(bytes, 16)?, STRUCT_BYTES)?;
        let mappings = end(properties, word(bytes, 20)?, PROPERTY_BYTES)?;
        let records = end(mappings, word(bytes, 28)?, MAPPING_BYTES)?;
        let text = end(records, word(bytes, 32)?, RECORD_BYTES)?;
        let blob = end(text, word(bytes, SNIFF_LEN - 8)?, 1)?;
        let instances = end(blob, word(bytes, SNIFF_LEN - 4)?, 1)?;
        bytes.get(instances..).ok_or("the actual instance tail")?;
        Ok(Self {
            structures,
            properties,
            mappings,
            records,
            text,
            blob,
            instances,
        })
    }

    pub(crate) fn string<'bytes>(
        &self,
        bytes: &'bytes [u8],
        start: usize,
        offset: u32,
    ) -> TestResult<&'bytes str> {
        let last = if start == self.blob {
            self.instances
        } else if start == self.text {
            self.blob
        } else {
            return Err("the requested original string table is absent".into());
        };
        let table = bytes.get(start..last).ok_or("the original string table")?;
        let tail = table
            .get(usize::try_from(offset)?..)
            .ok_or("original string position")?;
        let length = tail
            .iter()
            .position(|byte| *byte == 0)
            .ok_or("original string terminator")?;
        Ok(std::str::from_utf8(
            tail.get(..length).ok_or("original string bytes")?,
        )?)
    }
}

fn end(start: usize, count: u32, width: usize) -> TestResult<usize> {
    usize::try_from(count)?
        .checked_mul(width)
        .and_then(|length| start.checked_add(length))
        .ok_or_else(|| "original table position overflow".into())
}

pub(crate) fn word(bytes: &[u8], at: usize) -> TestResult<u32> {
    Ok(u32::from_le_bytes(
        bytes.get(at..at + 4).ok_or("original u32")?.try_into()?,
    ))
}

pub(crate) fn half(bytes: &[u8], at: usize) -> TestResult<u16> {
    Ok(u16::from_le_bytes(
        bytes.get(at..at + 2).ok_or("original u16")?.try_into()?,
    ))
}

pub(crate) fn write_half(bytes: &mut [u8], at: usize, value: u16) -> TestResult {
    bytes
        .get_mut(at..at + 2)
        .ok_or("fixture u16")?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

pub(crate) fn write_word(bytes: &mut [u8], at: usize, value: u32) -> TestResult {
    bytes
        .get_mut(at..at + 4)
        .ok_or("fixture u32")?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

pub(crate) fn original(bytes: &[u8], own: usize, padding: usize) -> TestResult {
    let tables = Tables::read(bytes)?;
    assert_eq!(word(bytes, 16)?, 2, "exactly two original struct rows");
    assert_eq!(
        word(bytes, 20)?,
        u32::try_from(2 * own)?,
        "the original property count is a full u32"
    );
    assert_eq!(
        (word(bytes, 28)?, word(bytes, 32)?),
        (2, 1),
        "actual data mappings and Record"
    );
    definitions(bytes, &tables, own, padding)?;
    properties(bytes, &tables, own)?;
    mappings(bytes, &tables)?;
    let instance = bytes
        .get(tables.instances..)
        .ok_or("the actual instance bytes")?;
    assert_eq!(
        instance.len(),
        2 * own + padding,
        "the actual source tail fills its declared instance"
    );
    for (index, actual) in instance.iter().enumerate() {
        let expected = if index < 2 * own {
            fixture::value(index, own)
        } else {
            0
        };
        assert_eq!(
            *actual, expected,
            "the independently checked original byte at slot{index}"
        );
    }
    Ok(())
}

fn definitions(bytes: &[u8], tables: &Tables, own: usize, padding: usize) -> TestResult {
    for (index, expected_name) in [BASE, DERIVED].into_iter().enumerate() {
        let at = tables.structures + index * STRUCT_BYTES;
        assert_eq!(
            tables.string(bytes, tables.blob, word(bytes, at)?)?,
            expected_name,
            "original struct name"
        );
        assert_eq!(
            word(bytes, at + 4)?,
            if index == 0 { u32::MAX } else { 0 },
            "original parent word"
        );
        assert_eq!(
            half(bytes, at + 8)?,
            u16::try_from(own)?,
            "each original property_count fits u16"
        );
        assert_eq!(
            half(bytes, at + 10)?,
            u16::try_from(index * own)?,
            "the actual original property-range start fits u16 without truncation"
        );
        let size = if index == 0 { own } else { 2 * own + padding };
        assert_eq!(
            word(bytes, at + 12)?,
            u32::try_from(size)?,
            "declared size is an independent u32"
        );
    }
    Ok(())
}

fn properties(bytes: &[u8], tables: &Tables, own: usize) -> TestResult {
    for index in 0..2 * own {
        let at = tables.properties + index * PROPERTY_BYTES;
        assert_eq!(
            tables.string(bytes, tables.blob, word(bytes, at)?)?,
            fixture::field_name(index, own),
            "every original physical property row and name"
        );
        assert_eq!(
            half(bytes, at + 4)?,
            u16::MAX,
            "scalar storage has no selected target"
        );
        assert_eq!(half(bytes, at + 6)?, 0x0006, "the original UInt8 type code");
        assert_eq!(
            (half(bytes, at + 8)?, half(bytes, at + 10)?),
            (0, 0),
            "scalar conversion and reserved word"
        );
    }
    Ok(())
}

fn mappings(bytes: &[u8], tables: &Tables) -> TestResult {
    for index in 0..2 {
        let at = tables.mappings + index * MAPPING_BYTES;
        assert_eq!(
            (word(bytes, at)?, word(bytes, at + 4)?),
            (u32::try_from(index)?, u32::try_from(index)?),
            "only the descendant contributes one actual stored instance"
        );
    }
    assert_eq!(
        word(bytes, tables.records + 12)?,
        1,
        "the actual Record's u32 struct index"
    );
    assert_eq!(
        bytes.get(tables.records + 16..tables.records + 32),
        Some([0x77; 16].as_slice()),
        "the actual stored Record GUID bytes"
    );
    assert_eq!(
        (
            half(bytes, tables.records + 32)?,
            half(bytes, tables.records + 34)?
        ),
        (0, 0),
        "the original Record instance is zero; unknown trailing metadata is explicitly zero"
    );
    assert_eq!(
        tables.string(bytes, tables.text, word(bytes, tables.records + 4)?)?,
        "source/wide-fields.xml",
        "the original stored filename"
    );
    Ok(())
}
