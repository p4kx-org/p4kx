// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::fixture::{Result, put};

pub(super) struct V2 {
    pub(super) bytes: Vec<u8>,
    pub(super) directory: usize,
    pub(super) names: usize,
    pub(super) trie: usize,
    pub(super) trailer: usize,
}

pub(super) fn v2(base: usize) -> Result<V2> {
    let mut bytes = b"abc".to_vec();
    for value in [0_u64, 3, 0, 0, u64::MAX, 2] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let directory = bytes.len();
    for index in 0..2 {
        let mut record = [0_u8; 204];
        if index == 1 {
            put(&mut record, 6, &crc32c::crc32c(b"abc").to_le_bytes())?;
            put(&mut record, 10, &3_u64.to_le_bytes())?;
            put(&mut record, 18, &3_u64.to_le_bytes())?;
            put(&mut record, 34, &6_u64.to_le_bytes())?;
            put(&mut record, 172, &[0x7d; 32])?;
        }
        put(&mut record, 42, &[0x80 + index; 128])?;
        bytes.extend_from_slice(&record);
    }
    let names = bytes.len();
    bytes.extend_from_slice(b"Data/\0Data/a.bin\0");
    let trie = bytes.len();
    bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
    let trailer = bytes.len();
    let base_names = match base {
        0 => 0,
        1 => 6,
        2 => 17,
        _ => return Err("invalid fixture split".into()),
    };
    let mut fixed = [0_u8; 175];
    for (offset, value) in [
        (0, base),
        (8, 2 - base),
        (16, directory),
        (24, base * 204),
        (32, (2 - base) * 204),
        (40, names),
        (48, base_names),
        (56, 17 - base_names),
        (64, 3),
        (72, 2),
        (80, trie),
        (88, 4),
        (96, 4096),
    ] {
        put(&mut fixed, offset, &u64::try_from(value)?.to_le_bytes())?;
    }
    put(&mut fixed, 105, &[0x19; 32])?;
    put(&mut fixed, 137, &[0xe1; 32])?;
    put(&mut fixed, 169, &[2, 0, b'J', b'i', b'j', b'i'])?;
    bytes.extend_from_slice(&fixed);
    Ok(V2 {
        bytes,
        directory,
        names,
        trie,
        trailer,
    })
}
