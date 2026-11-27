// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::fixture::{Result, put};

pub(super) struct Zip {
    pub(super) bytes: Vec<u8>,
    pub(super) records: [usize; 2],
    pub(super) directory: usize,
    pub(super) directory_len: usize,
    pub(super) zip64: Option<usize>,
    pub(super) locator: Option<usize>,
    pub(super) end: usize,
}

pub(super) fn zip(extra: &[u8], comment: &[u8], zip64: bool) -> Result<Zip> {
    let mut bytes = Vec::new();
    local(&mut bytes, b"Data/", b"")?;
    let offset = bytes.len();
    local(&mut bytes, b"Data/a.bin", b"abc")?;
    if zip64 {
        installation(&mut bytes);
    }
    let directory = bytes.len();
    let records = [directory, directory + 51];
    central(&mut bytes, b"Data/", b"", 0, &[], &[])?;
    central(&mut bytes, b"Data/a.bin", b"abc", offset, extra, comment)?;
    let directory_len = bytes.len() - directory;
    let zip64_offset = if zip64 { Some(bytes.len()) } else { None };
    let locator = if zip64 {
        Some(append_zip64(&mut bytes, directory, directory_len)?)
    } else {
        None
    };
    let end = bytes.len();
    append_end(&mut bytes, directory, directory_len, zip64)?;
    Ok(Zip {
        bytes,
        records,
        directory,
        directory_len,
        zip64: zip64_offset,
        locator,
        end,
    })
}

fn local(bytes: &mut Vec<u8>, name: &[u8], payload: &[u8]) -> Result {
    let mut fixed = [0_u8; 30];
    put(&mut fixed, 0, &[0x50, 0x4b, 0x03, 0x04])?;
    put(&mut fixed, 4, &20_u16.to_le_bytes())?;
    put(&mut fixed, 14, &crc32fast::hash(payload).to_le_bytes())?;
    put(&mut fixed, 18, &u32::try_from(payload.len())?.to_le_bytes())?;
    put(&mut fixed, 22, &u32::try_from(payload.len())?.to_le_bytes())?;
    put(&mut fixed, 26, &u16::try_from(name.len())?.to_le_bytes())?;
    bytes.extend_from_slice(&fixed);
    bytes.extend_from_slice(name);
    bytes.extend_from_slice(payload);
    Ok(())
}

fn central(
    bytes: &mut Vec<u8>,
    name: &[u8],
    payload: &[u8],
    offset: usize,
    extra: &[u8],
    comment: &[u8],
) -> Result {
    let mut fixed = [0_u8; 46];
    put(&mut fixed, 0, &[0x50, 0x4b, 0x01, 0x02])?;
    put(&mut fixed, 4, &0x0314_u16.to_le_bytes())?;
    put(&mut fixed, 6, &20_u16.to_le_bytes())?;
    put(&mut fixed, 16, &crc32fast::hash(payload).to_le_bytes())?;
    put(&mut fixed, 20, &u32::try_from(payload.len())?.to_le_bytes())?;
    put(&mut fixed, 24, &u32::try_from(payload.len())?.to_le_bytes())?;
    put(&mut fixed, 28, &u16::try_from(name.len())?.to_le_bytes())?;
    put(&mut fixed, 30, &u16::try_from(extra.len())?.to_le_bytes())?;
    put(&mut fixed, 32, &u16::try_from(comment.len())?.to_le_bytes())?;
    put(&mut fixed, 42, &u32::try_from(offset)?.to_le_bytes())?;
    bytes.extend_from_slice(&fixed);
    bytes.extend_from_slice(name);
    bytes.extend_from_slice(extra);
    bytes.extend_from_slice(comment);
    Ok(())
}

fn installation(bytes: &mut Vec<u8>) {
    for (progress, reserved) in [(0_u64, 0_u32), (3, 0x1234_5678)] {
        bytes.extend_from_slice(&progress.to_le_bytes());
        bytes.extend_from_slice(&reserved.to_le_bytes());
    }
    for (offset, len) in [(0_u64, 0_u64), (u64::MAX, 8)] {
        bytes.extend_from_slice(&offset.to_le_bytes());
        bytes.extend_from_slice(&len.to_le_bytes());
    }
}

fn append_zip64(bytes: &mut Vec<u8>, directory: usize, directory_len: usize) -> Result<usize> {
    let offset = u64::try_from(bytes.len())?;
    let mut fixed = [0_u8; 56];
    put(&mut fixed, 0, &[0x50, 0x4b, 0x06, 0x06])?;
    put(&mut fixed, 4, &48_u64.to_le_bytes())?;
    put(&mut fixed, 12, &0x031e_u16.to_le_bytes())?;
    put(&mut fixed, 14, &45_u16.to_le_bytes())?;
    put(&mut fixed, 16, &17_u32.to_le_bytes())?;
    put(&mut fixed, 20, &19_u32.to_le_bytes())?;
    put(&mut fixed, 24, &1_u64.to_le_bytes())?;
    put(&mut fixed, 32, &2_u64.to_le_bytes())?;
    put(&mut fixed, 40, &u64::try_from(directory_len)?.to_le_bytes())?;
    put(&mut fixed, 48, &u64::try_from(directory)?.to_le_bytes())?;
    bytes.extend_from_slice(&fixed);
    bytes.extend_from_slice(&[0x00, 0xfe, 0xff, 0x7a]);
    let locator = bytes.len();
    let mut fixed = [0_u8; 20];
    put(&mut fixed, 0, &[0x50, 0x4b, 0x06, 0x07])?;
    put(&mut fixed, 4, &23_u32.to_le_bytes())?;
    put(&mut fixed, 8, &offset.to_le_bytes())?;
    put(&mut fixed, 16, &29_u32.to_le_bytes())?;
    bytes.extend_from_slice(&fixed);
    Ok(locator)
}

fn append_end(bytes: &mut Vec<u8>, directory: usize, directory_len: usize, zip64: bool) -> Result {
    let comment = if zip64 {
        b"CIG\0\x01\0\0\x04\x02\0\0\0\0\0\0\0".as_slice()
    } else {
        b"opaque\0\xffcomment".as_slice()
    };
    let mut fixed = [0_u8; 22];
    put(&mut fixed, 0, &[0x50, 0x4b, 0x05, 0x06])?;
    put(&mut fixed, 4, &7_u16.to_le_bytes())?;
    put(&mut fixed, 6, &11_u16.to_le_bytes())?;
    put(&mut fixed, 8, &1_u16.to_le_bytes())?;
    put(&mut fixed, 10, &2_u16.to_le_bytes())?;
    put(&mut fixed, 12, &u32::try_from(directory_len)?.to_le_bytes())?;
    put(&mut fixed, 16, &u32::try_from(directory)?.to_le_bytes())?;
    put(&mut fixed, 20, &u16::try_from(comment.len())?.to_le_bytes())?;
    bytes.extend_from_slice(&fixed);
    bytes.extend_from_slice(comment);
    bytes.extend_from_slice(b"PAD");
    Ok(())
}
