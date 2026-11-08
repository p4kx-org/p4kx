// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_archive::{Archive, Entry, OpenOptions};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

pub(super) type FixtureResult<T> = Result<T, Box<dyn Error>>;

pub(super) const PAYLOAD: &[u8] = b"synthetic payload bytes";

const CENTRAL_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];
const CENTRAL_HEADER_LEN: usize = 46;
const ZIP_FLAGS_OFFSET: usize = 8;
const NAME_LEN_OFFSET: usize = 28;
const EXTRA_LEN_OFFSET: usize = 30;
const BLOCK_HEADER_LEN: usize = 4;
const CIG_ENCRYPTION: u16 = 0x5002;
const CIG_HASH: u16 = 0x5003;
const WORD_BLOCK_LEN: u16 = 6;
const IGNORED_BLOCK: u16 = 0x7f7f;

struct Block {
    header_offset: usize,
    id: u16,
    total_len: u16,
}

pub(super) fn open(
    bytes: &[u8],
    options: OpenOptions,
) -> FixtureResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("fixture.p4k");
    fs::write(&path, bytes)?;
    let archive = Archive::open_with(&path, options)?;
    assert_eq!(
        fs::read(&path)?,
        bytes,
        "opening retains every fixture byte"
    );
    Ok((directory, archive))
}

pub(super) fn entry(archive: &Archive, index: usize) -> FixtureResult<&Entry> {
    archive
        .entries()
        .get(index)
        .ok_or_else(|| "Entry missing".into())
}

pub(super) fn v1(entry: TestEntry) -> Vec<u8> {
    ArchiveBuilder::new().entry(entry).build()
}

pub(super) fn missing_declaration(mut bytes: Vec<u8>) -> FixtureResult<Vec<u8>> {
    for block in blocks(&bytes)? {
        if block.id == CIG_ENCRYPTION {
            write_word(&mut bytes, block.header_offset, IGNORED_BLOCK)?;
        }
    }
    Ok(bytes)
}

pub(super) fn additional_declarations(
    mut entry: TestEntry,
    values: &[u16],
) -> FixtureResult<Vec<u8>> {
    for value in values {
        entry = entry.with_extra_hash_block(&value.to_le_bytes());
    }
    let mut bytes = v1(entry);
    for block in blocks(&bytes)? {
        if block.id == CIG_HASH && block.total_len == WORD_BLOCK_LEN {
            write_word(&mut bytes, block.header_offset, CIG_ENCRYPTION)?;
        }
    }
    Ok(bytes)
}

pub(super) fn cig_offsets(bytes: &[u8]) -> FixtureResult<Vec<u64>> {
    blocks(bytes)?
        .into_iter()
        .filter(|block| block.id == CIG_ENCRYPTION)
        .map(|block| Ok(u64::try_from(block.header_offset + BLOCK_HEADER_LEN)?))
        .collect()
}

pub(super) fn word(bytes: &[u8], offset: usize) -> FixtureResult<u16> {
    let raw: [u8; 2] = bytes
        .get(offset..offset + 2)
        .ok_or("word outside fixture")?
        .try_into()?;
    Ok(u16::from_le_bytes(raw))
}

fn write_word(bytes: &mut [u8], offset: usize, id: u16) -> FixtureResult<()> {
    bytes
        .get_mut(offset..offset + 2)
        .ok_or("block outside fixture")?
        .copy_from_slice(&id.to_le_bytes());
    Ok(())
}

fn blocks(bytes: &[u8]) -> FixtureResult<Vec<Block>> {
    let record = record_offset(bytes)?;
    let name_len = usize::from(word(bytes, record + NAME_LEN_OFFSET)?);
    let extra_len = usize::from(word(bytes, record + EXTRA_LEN_OFFSET)?);
    let mut offset = record + CENTRAL_HEADER_LEN + name_len;
    let end = offset + extra_len;
    let mut result = Vec::new();
    while offset < end {
        let id = word(bytes, offset)?;
        let total_len = word(bytes, offset + 2)?;
        let length = usize::from(total_len);
        if length < BLOCK_HEADER_LEN || offset + length > end {
            return Err("invalid authored block framing".into());
        }
        result.push(Block {
            header_offset: offset,
            id,
            total_len,
        });
        offset += length;
    }
    Ok(result)
}

pub(super) fn zip_flags(mut bytes: Vec<u8>, flags: u16) -> FixtureResult<Vec<u8>> {
    let offset = record_offset(&bytes)? + ZIP_FLAGS_OFFSET;
    write_word(&mut bytes, offset, flags)?;
    Ok(bytes)
}

fn record_offset(bytes: &[u8]) -> FixtureResult<usize> {
    bytes
        .windows(CENTRAL_SIGNATURE.len())
        .position(|window| window == CENTRAL_SIGNATURE)
        .ok_or_else(|| "fixture directory missing".into())
}
