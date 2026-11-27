// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::fixture::{self, FixtureResult, Words};

#[derive(Clone, Copy)]
pub(super) enum Layout {
    Zip,
    P4k,
}

pub(super) struct Fixture {
    pub(super) bytes: Vec<u8>,
    pub(super) directory_end: usize,
}

struct Record {
    name: String,
    words: Words,
    offset: u32,
    checksum: u32,
    size: u32,
}

pub(super) fn fixture(stamps: &[Words], layout: Layout) -> FixtureResult<Fixture> {
    let mut bytes = Vec::new();
    let mut records = vec![record(
        &mut bytes,
        "Data/".to_owned(),
        Words {
            date: 0x0021,
            time: 0,
        },
        b"",
        layout,
    )?];
    for (index, words) in stamps.iter().copied().enumerate() {
        records.push(record(
            &mut bytes,
            format!("Data/item-{index}.bin"),
            words,
            b"abc",
            layout,
        )?);
    }
    let directory = bytes.len();
    for entry in &records {
        central(&mut bytes, entry)?;
    }
    let directory_end = bytes.len();
    let count = u16::try_from(records.len())?;
    let size = u32::try_from(directory_end - directory)?;
    if matches!(layout, Layout::P4k) {
        zip64_end(&mut bytes, directory, size, count)?;
    }
    let mut end = [0_u8; 22];
    fixture::put(&mut end, 0, &[0x50, 0x4b, 0x05, 0x06])?;
    fixture::put(&mut end, 8, &count.to_le_bytes())?;
    fixture::put(&mut end, 10, &count.to_le_bytes())?;
    fixture::put(&mut end, 12, &size.to_le_bytes())?;
    fixture::put(&mut end, 16, &u32::try_from(directory)?.to_le_bytes())?;
    bytes.extend_from_slice(&end);
    Ok(Fixture {
        bytes,
        directory_end,
    })
}

fn record(
    bytes: &mut Vec<u8>,
    name: String,
    words: Words,
    payload: &[u8],
    layout: Layout,
) -> FixtureResult<Record> {
    let offset = u32::try_from(bytes.len())?;
    let size = u32::try_from(payload.len())?;
    let checksum = match layout {
        Layout::Zip => crc32fast::hash(payload),
        Layout::P4k => crc32c::crc32c(payload),
    };
    let mut header = [0_u8; 30];
    let signature = match layout {
        Layout::Zip => [0x50, 0x4b, 0x03, 0x04],
        Layout::P4k => [0x50, 0x4b, 0x03, 0x14],
    };
    fixture::put(&mut header, 0, &signature)?;
    fixture::put(&mut header, 4, &20_u16.to_le_bytes())?;
    fixture::put(&mut header, 12, &0x0021_u16.to_le_bytes())?;
    fixture::put(&mut header, 14, &checksum.to_le_bytes())?;
    fixture::put(&mut header, 18, &size.to_le_bytes())?;
    fixture::put(&mut header, 22, &size.to_le_bytes())?;
    fixture::put(&mut header, 26, &u16::try_from(name.len())?.to_le_bytes())?;
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(name.as_bytes());
    bytes.extend_from_slice(payload);
    Ok(Record {
        name,
        words,
        offset,
        checksum,
        size,
    })
}

fn central(bytes: &mut Vec<u8>, record: &Record) -> FixtureResult<()> {
    let mut header = [0_u8; 46];
    fixture::put(&mut header, 0, &[0x50, 0x4b, 0x01, 0x02])?;
    fixture::put(&mut header, 4, &20_u16.to_le_bytes())?;
    fixture::put(&mut header, 6, &20_u16.to_le_bytes())?;
    fixture::put(&mut header, 12, &record.words.time.to_le_bytes())?;
    fixture::put(&mut header, 14, &record.words.date.to_le_bytes())?;
    fixture::put(&mut header, 16, &record.checksum.to_le_bytes())?;
    fixture::put(&mut header, 20, &record.size.to_le_bytes())?;
    fixture::put(&mut header, 24, &record.size.to_le_bytes())?;
    fixture::put(
        &mut header,
        28,
        &u16::try_from(record.name.len())?.to_le_bytes(),
    )?;
    fixture::put(&mut header, 42, &record.offset.to_le_bytes())?;
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(record.name.as_bytes());
    Ok(())
}

fn zip64_end(bytes: &mut Vec<u8>, directory: usize, size: u32, count: u16) -> FixtureResult<()> {
    let offset = u64::try_from(bytes.len())?;
    let mut record = [0_u8; 56];
    fixture::put(&mut record, 0, &[0x50, 0x4b, 0x06, 0x06])?;
    fixture::put(&mut record, 4, &44_u64.to_le_bytes())?;
    fixture::put(&mut record, 12, &45_u16.to_le_bytes())?;
    fixture::put(&mut record, 14, &45_u16.to_le_bytes())?;
    fixture::put(&mut record, 24, &u64::from(count).to_le_bytes())?;
    fixture::put(&mut record, 32, &u64::from(count).to_le_bytes())?;
    fixture::put(&mut record, 40, &u64::from(size).to_le_bytes())?;
    fixture::put(&mut record, 48, &u64::try_from(directory)?.to_le_bytes())?;
    bytes.extend_from_slice(&record);
    let mut locator = [0_u8; 20];
    fixture::put(&mut locator, 0, &[0x50, 0x4b, 0x06, 0x07])?;
    fixture::put(&mut locator, 8, &offset.to_le_bytes())?;
    fixture::put(&mut locator, 16, &1_u32.to_le_bytes())?;
    bytes.extend_from_slice(&locator);
    Ok(())
}
