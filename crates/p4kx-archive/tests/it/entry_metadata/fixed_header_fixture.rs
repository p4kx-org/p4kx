// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::fixture::{self, FixtureResult, Words};
use super::{v2, zip};

pub(super) const STAMP: Words = Words {
    date: 0x585d,
    time: 0xbf7d,
};
pub(super) const ZERO_STAMP: Words = Words { date: 0, time: 0 };
pub(super) const FOLDER_STAMP: Words = Words {
    date: 0x0021,
    time: 0,
};
pub(super) const FOLDER_WORDS: HeaderWords = HeaderWords {
    made_by: 0xcafe,
    needed: 0xbeef,
    flags: 0x8001,
    method: 0,
    disk: 0xffff,
    internal: 0xa55a,
    external: 0x8123_4567,
};
pub(super) const FILE_WORDS: HeaderWords = HeaderWords {
    made_by: 0x03ff,
    needed: 0x02aa,
    flags: 0xf00d,
    method: 0xd00d,
    disk: 0x1234,
    internal: 0x9669,
    external: 0xf123_4567,
};
pub(super) const ZERO_WORDS: HeaderWords = HeaderWords {
    made_by: 0,
    needed: 0,
    flags: 0,
    method: 0,
    disk: 0,
    internal: 0,
    external: 0,
};
pub(super) const V2_WORDS: [(u16, u16); 4] = [(0xd00d, 0xffff), (100, 1), (8, 0), (0, 0)];
const V2_RECORD_LEN: usize = 204;
const V2_ENCRYPTION_OFFSET: usize = 170;
const V2_PAYLOAD_BYTES: usize = 3;
const V2_PROGRESS_BYTES: usize = 8;

#[derive(Clone, Copy)]
pub(super) struct HeaderWords {
    pub(super) made_by: u16,
    pub(super) needed: u16,
    pub(super) flags: u16,
    pub(super) method: u16,
    pub(super) disk: u16,
    pub(super) internal: u16,
    pub(super) external: u32,
}

pub(super) fn central_offsets(bytes: &[u8]) -> Vec<usize> {
    bytes
        .windows(4)
        .enumerate()
        .filter_map(|(index, word)| (word == [0x50, 0x4b, 0x01, 0x02]).then_some(index))
        .collect()
}

fn stamp(bytes: &mut [u8], at: usize, words: HeaderWords) -> FixtureResult<()> {
    for (offset, word) in [
        (4, words.made_by),
        (6, words.needed),
        (8, words.flags),
        (10, words.method),
        (34, words.disk),
        (36, words.internal),
    ] {
        fixture::put(bytes, at + offset, &word.to_le_bytes())?;
    }
    fixture::put(bytes, at + 38, &words.external.to_le_bytes())
}

pub(super) fn zip_source(
    layout: zip::Layout,
    words: HeaderWords,
    timestamp: Words,
) -> FixtureResult<zip::Fixture> {
    let mut source = zip::fixture(&[timestamp], layout)?;
    let offsets = central_offsets(&source.bytes);
    assert_eq!(offsets.len(), 2, "a folder precedes the one file record");
    stamp(
        &mut source.bytes,
        *offsets.first().ok_or("folder record")?,
        FOLDER_WORDS,
    )?;
    stamp(
        &mut source.bytes,
        *offsets.get(1).ok_or("file record")?,
        words,
    )?;
    Ok(source)
}

pub(super) fn v2_source(timestamp: Words, base: usize) -> FixtureResult<Vec<u8>> {
    let mut bytes = v2::fixture(&[timestamp; V2_WORDS.len()], base)?;
    let directory = V2_WORDS.len() * (V2_PAYLOAD_BYTES + V2_PROGRESS_BYTES);
    let records = V2_WORDS.iter().enumerate();
    for (index, (method, encryption)) in records {
        let record = directory + index * V2_RECORD_LEN;
        fixture::put(&mut bytes, record, &method.to_le_bytes())?;
        fixture::put(
            &mut bytes,
            record + V2_ENCRYPTION_OFFSET,
            &encryption.to_le_bytes(),
        )?;
    }
    Ok(bytes)
}

pub(super) fn v2_folder_source(timestamp: Words, base: usize) -> FixtureResult<Vec<u8>> {
    let mut bytes = v2_source(timestamp, base)?;
    let payload_end = V2_WORDS.len() * V2_PAYLOAD_BYTES;
    let directory = payload_end + V2_WORDS.len() * V2_PROGRESS_BYTES;
    let names = directory + V2_WORDS.len() * V2_RECORD_LEN;
    fixture::put(&mut bytes, payload_end, &0_u64.to_le_bytes())?;
    fixture::put(&mut bytes, directory, &0_u16.to_le_bytes())?;
    fixture::put(&mut bytes, directory + 6, &0_u32.to_le_bytes())?;
    fixture::put(&mut bytes, directory + 10, &0_u64.to_le_bytes())?;
    fixture::put(&mut bytes, directory + 18, &0_u64.to_le_bytes())?;
    fixture::put(
        &mut bytes,
        directory + V2_ENCRYPTION_OFFSET,
        &0_u16.to_le_bytes(),
    )?;
    fixture::put(&mut bytes, names, b"Data/\0")?;
    Ok(bytes)
}
