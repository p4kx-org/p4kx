// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::fixture::{self, FixtureResult, Words};

const RECORD_LEN: usize = 204;
const TRAILER_LEN: usize = 175;

pub(super) fn fixture(stamps: &[Words], base: usize) -> FixtureResult<Vec<u8>> {
    let payload_end = stamps.len() * 3;
    let mut bytes = b"abc".repeat(stamps.len());
    for _ in stamps {
        bytes.extend_from_slice(&3_u64.to_le_bytes());
    }
    let directory = bytes.len();
    let mut names = Vec::new();
    let mut base_names = 0_usize;
    for (index, words) in stamps.iter().enumerate() {
        let mut record = [0_u8; RECORD_LEN];
        fixture::put(&mut record, 2, &words.time.to_le_bytes())?;
        fixture::put(&mut record, 4, &words.date.to_le_bytes())?;
        fixture::put(&mut record, 6, &crc32c::crc32c(b"abc").to_le_bytes())?;
        fixture::put(&mut record, 10, &3_u64.to_le_bytes())?;
        fixture::put(&mut record, 18, &3_u64.to_le_bytes())?;
        fixture::put(&mut record, 26, &u64::try_from(index * 3)?.to_le_bytes())?;
        fixture::put(&mut record, 34, &u64::try_from(names.len())?.to_le_bytes())?;
        bytes.extend_from_slice(&record);
        names.extend_from_slice(format!("Data/item-{index}.bin\0").as_bytes());
        if index + 1 == base {
            base_names = names.len();
        }
    }
    let names_offset = bytes.len();
    bytes.extend_from_slice(&names);
    let mut trailer = [0_u8; TRAILER_LEN];
    for (offset, value) in [
        (0, base),
        (8, stamps.len() - base),
        (16, directory),
        (24, base * RECORD_LEN),
        (32, (stamps.len() - base) * RECORD_LEN),
        (40, names_offset),
        (48, base_names),
        (56, names.len() - base_names),
        (64, payload_end),
    ] {
        fixture::put(&mut trailer, offset, &u64::try_from(value)?.to_le_bytes())?;
    }
    fixture::put(&mut trailer, 169, &[2, 0, b'J', b'i', b'j', b'i'])?;
    bytes.extend_from_slice(&trailer);
    Ok(bytes)
}
