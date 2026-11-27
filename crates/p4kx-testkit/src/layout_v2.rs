// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::TestEntry;
use crate::bytes::{len_u64, put_u16, put_u32, put_u64};

const RECORD_SIGNATURE_LEN: usize = 128;
const TRAILER_MANIFEST_HASH_LEN: usize = 64;
const PHYSICAL_SECTOR_SIZE: u64 = 4096;
const LAYOUT_VERSION: u16 = 2;
const LAYOUT_MAGIC: u32 = 0x696A_694A;

/// Writes `entries` as a v2 Archive: the data of every Entry without local headers, the 204-byte
/// directory records, the name table and the 175-byte `Jiji` trailer.
pub(crate) fn build(entries: &[TestEntry]) -> Vec<u8> {
    let mut out = Vec::new();
    let data_offsets: Vec<u64> = entries
        .iter()
        .map(|entry| {
            let offset = len_u64(&out);
            out.extend_from_slice(&entry.payload);
            offset
        })
        .collect();
    let end_of_payload = len_u64(&out);
    for entry in entries {
        put_u64(&mut out, len_u64(&entry.payload));
    }
    let mut names = Vec::new();
    let directory_offset = len_u64(&out);
    for (entry, data_offset) in entries.iter().zip(data_offsets) {
        let name_offset = len_u64(&names);
        names.extend_from_slice(entry.path_bytes());
        names.push(0);
        write_record(&mut out, entry, data_offset, name_offset);
    }
    let directory_size = len_u64(&out) - directory_offset;
    let names_offset = len_u64(&out);
    out.extend_from_slice(&names);
    let trailer = Trailer {
        count: len_u64(entries),
        directory_offset,
        directory_size,
        names_offset,
        names_size: len_u64(&names),
        end_of_payload,
    };
    write_trailer(&mut out, &trailer);
    out
}

struct Trailer {
    count: u64,
    directory_offset: u64,
    directory_size: u64,
    names_offset: u64,
    names_size: u64,
    end_of_payload: u64,
}

fn write_record(out: &mut Vec<u8>, entry: &TestEntry, data_offset: u64, name_offset: u64) {
    put_u16(out, entry.method);
    put_u16(out, 0);
    put_u16(out, 0);
    put_u32(out, entry.crc32c);
    put_u64(out, len_u64(&entry.payload));
    put_u64(out, entry.uncompressed_size);
    put_u64(out, data_offset);
    put_u64(out, name_offset);
    out.extend_from_slice(&[0; RECORD_SIGNATURE_LEN]);
    put_u16(out, entry.encryption_value());
    out.extend_from_slice(&entry.sha256);
}

fn write_trailer(out: &mut Vec<u8>, trailer: &Trailer) {
    put_u64(out, trailer.count);
    put_u64(out, 0);
    put_u64(out, trailer.directory_offset);
    put_u64(out, trailer.directory_size);
    put_u64(out, 0);
    put_u64(out, trailer.names_offset);
    put_u64(out, trailer.names_size);
    put_u64(out, 0);
    put_u64(out, trailer.end_of_payload);
    put_u64(out, 0);
    put_u64(out, 0);
    put_u64(out, 0);
    put_u64(out, PHYSICAL_SECTOR_SIZE);
    out.push(0);
    out.extend_from_slice(&[0; TRAILER_MANIFEST_HASH_LEN]);
    put_u16(out, LAYOUT_VERSION);
    put_u32(out, LAYOUT_MAGIC);
}
