// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, OpenOptions};

use super::support::TestResult;

pub(super) struct Fixture {
    pub(super) bytes: Vec<u8>,
    pub(super) records: Vec<usize>,
    pub(super) installation: usize,
    pub(super) free: usize,
    pub(super) end: usize,
}

pub(super) fn open(
    root: &Path,
    bytes: &[u8],
    recover: bool,
) -> Result<Archive, Box<dyn std::error::Error>> {
    let path = root.join("layout.p4k");
    fs::write(&path, bytes)?;
    Ok(Archive::open_with(
        &path,
        OpenOptions::new().recover(recover),
    )?)
}

pub(super) fn put(bytes: &mut [u8], offset: usize, value: &[u8]) -> TestResult {
    let end = offset
        .checked_add(value.len())
        .ok_or("fixture offset overflow")?;
    bytes
        .get_mut(offset..end)
        .ok_or("authored fixture extent")?
        .copy_from_slice(value);
    Ok(())
}

pub(super) fn zip(extra: &[u8]) -> Result<Fixture, Box<dyn std::error::Error>> {
    let names: [&[u8]; 2] = [b"Data/", b"Data/empty.bin"];
    let mut bytes = Vec::new();
    let offsets = append_locals(&mut bytes, &names)?;
    let installation = bytes.len();
    bytes.extend_from_slice(&[0; 24]);
    let free = bytes.len();
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes.extend_from_slice(&1_u64.to_le_bytes());
    let directory = bytes.len();
    let mut records = Vec::new();
    for (index, name) in names.into_iter().enumerate() {
        records.push(bytes.len());
        let comment: &[u8] = if index == 1 { &[0xff, 0, 0xfe] } else { &[] };
        let extra = if index == 1 { extra } else { &[] };
        let mut record = [0; 46];
        put(&mut record, 0, b"PK\x01\x02")?;
        put(&mut record, 4, &0x0314_u16.to_le_bytes())?;
        put(&mut record, 6, &20_u16.to_le_bytes())?;
        put(&mut record, 8, &0x0800_u16.to_le_bytes())?;
        put(&mut record, 12, &0xbf7d_u16.to_le_bytes())?;
        put(&mut record, 14, &0x285d_u16.to_le_bytes())?;
        put(&mut record, 28, &u16::try_from(name.len())?.to_le_bytes())?;
        put(&mut record, 30, &u16::try_from(extra.len())?.to_le_bytes())?;
        put(
            &mut record,
            32,
            &u16::try_from(comment.len())?.to_le_bytes(),
        )?;
        put(
            &mut record,
            42,
            &u32::try_from(*offsets.get(index).ok_or("local offset")?)?.to_le_bytes(),
        )?;
        bytes.extend_from_slice(&record);
        bytes.extend_from_slice(name);
        bytes.extend_from_slice(extra);
        bytes.extend_from_slice(comment);
    }
    let directory_size = bytes.len() - directory;
    append_zip64(&mut bytes, directory, directory_size)?;
    let end = bytes.len();
    append_end(&mut bytes, directory, directory_size)?;
    Ok(Fixture {
        bytes,
        records,
        installation,
        free,
        end,
    })
}

fn append_zip64(bytes: &mut Vec<u8>, directory: usize, directory_size: usize) -> TestResult {
    let offset = bytes.len();
    let mut fixed = [0; 56];
    put(&mut fixed, 0, b"PK\x06\x06")?;
    put(&mut fixed, 4, &49_u64.to_le_bytes())?;
    put(&mut fixed, 12, &0x031e_u16.to_le_bytes())?;
    put(&mut fixed, 14, &45_u16.to_le_bytes())?;
    put(&mut fixed, 16, &11_u32.to_le_bytes())?;
    put(&mut fixed, 20, &13_u32.to_le_bytes())?;
    put(&mut fixed, 24, &1_u64.to_le_bytes())?;
    put(&mut fixed, 32, &2_u64.to_le_bytes())?;
    put(
        &mut fixed,
        40,
        &u64::try_from(directory_size)?.to_le_bytes(),
    )?;
    put(&mut fixed, 48, &u64::try_from(directory)?.to_le_bytes())?;
    bytes.extend_from_slice(&fixed);
    bytes.extend_from_slice(&[0xa1, 0, 0xfe, 0xff, 0xb2]);
    let mut locator = [0; 20];
    put(&mut locator, 0, b"PK\x06\x07")?;
    put(&mut locator, 4, &17_u32.to_le_bytes())?;
    put(&mut locator, 8, &u64::try_from(offset)?.to_le_bytes())?;
    put(&mut locator, 16, &19_u32.to_le_bytes())?;
    bytes.extend_from_slice(&locator);
    Ok(())
}

pub(super) fn v2() -> Result<Fixture, Box<dyn std::error::Error>> {
    let mut bytes = vec![0; 16];
    let free = bytes.len();
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes.extend_from_slice(&1_u64.to_le_bytes());
    let directory = bytes.len();
    let records = vec![directory, directory + 204];
    for (index, name_offset) in [0_u64, 6].into_iter().enumerate() {
        let mut record = [0; 204];
        put(&mut record, 2, &0xbf7d_u16.to_le_bytes())?;
        put(&mut record, 4, &0x285d_u16.to_le_bytes())?;
        put(&mut record, 34, &name_offset.to_le_bytes())?;
        put(&mut record, 42, &[u8::try_from(index)? + 0x85; 128])?;
        if index == 1 {
            put(&mut record, 172, &[0x6a; 32])?;
        }
        bytes.extend_from_slice(&record);
    }
    let names = bytes.len();
    bytes.extend_from_slice(b"Data/\0Data/empty.bin\0");
    let trie = bytes.len();
    bytes.extend_from_slice(&[0x00, 0xee, 0xff, 0x73]);
    let end = bytes.len();
    let mut trailer = [0; 175];
    for (position, value) in [
        (0, 1),
        (8, 1),
        (16, directory),
        (24, 204),
        (32, 204),
        (40, names),
        (48, 6),
        (56, 15),
        (64, 0),
        (72, 1),
        (80, trie),
        (88, 4),
        (96, 8192),
    ] {
        put(&mut trailer, position, &u64::try_from(value)?.to_le_bytes())?;
    }
    put(&mut trailer, 105, &[0x13; 32])?;
    put(&mut trailer, 137, &[0xdb; 32])?;
    put(&mut trailer, 169, &[2, 0, b'J', b'i', b'j', b'i'])?;
    bytes.extend_from_slice(&trailer);
    Ok(Fixture {
        bytes,
        records,
        installation: 0,
        free,
        end,
    })
}

fn append_locals(
    bytes: &mut Vec<u8>,
    names: &[&[u8]],
) -> Result<Vec<usize>, Box<dyn std::error::Error>> {
    let mut offsets = Vec::new();
    for name in names {
        offsets.push(bytes.len());
        let mut local = [0; 30];
        put(&mut local, 0, b"PK\x03\x04")?;
        put(&mut local, 4, &20_u16.to_le_bytes())?;
        put(&mut local, 26, &u16::try_from(name.len())?.to_le_bytes())?;
        bytes.extend_from_slice(&local);
        bytes.extend_from_slice(name);
    }
    Ok(offsets)
}

fn append_end(bytes: &mut Vec<u8>, directory: usize, directory_size: usize) -> TestResult {
    let mut fixed = [0; 22];
    put(&mut fixed, 0, b"PK\x05\x06")?;
    put(&mut fixed, 4, &7_u16.to_le_bytes())?;
    put(&mut fixed, 6, &9_u16.to_le_bytes())?;
    put(&mut fixed, 8, &1_u16.to_le_bytes())?;
    put(&mut fixed, 10, &2_u16.to_le_bytes())?;
    put(
        &mut fixed,
        12,
        &u32::try_from(directory_size)?.to_le_bytes(),
    )?;
    put(&mut fixed, 16, &u32::try_from(directory)?.to_le_bytes())?;
    put(&mut fixed, 20, &16_u16.to_le_bytes())?;
    bytes.extend_from_slice(&fixed);
    bytes.extend_from_slice(b"CIG\0\x01\0\0\x08\x01\0\0\0\0\0\0\0");
    bytes.extend_from_slice(&[0xf1, 0, 0xff]);
    Ok(())
}
