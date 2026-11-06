// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::{ByteReader, ParseError};

use crate::entry::stated_sha256;
use crate::format_error::FormatError;
use crate::issue::{Directory, IssueLog, decode_path};
use crate::{Checksum, Compression, Entry, EntryIssue};

const CENTRAL_HEADER_SIGNATURE: u32 = 0x0201_4B50;
const CENTRAL_HEADER_LEN: usize = 46;
const VERSIONS_AND_FLAGS_LEN: usize = 6;
const TIME_AND_DATE_LEN: usize = 4;
const DISK_AND_ATTRIBUTES_LEN: usize = 8;
const ZIP64_MARKER_32: u32 = 0xFFFF_FFFF;
const BLOCK_ZIP64: u16 = 0x0001;
const BLOCK_CIG_ENCRYPTION: u16 = 0x5002;
const BLOCK_CIG_HASH: u16 = 0x5003;
const BLOCK_HEADER_LEN: usize = 4;
const ENCRYPTION_BLOCK_LEN: u16 = 6;
const SHA256_LEN: usize = 32;

struct FixedFields {
    method: u16,
    crc32c: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    local_header_offset: u32,
    name_len: usize,
    extra_len: usize,
    comment_len: usize,
}

struct ExtraFields {
    compressed_size: u64,
    uncompressed_size: u64,
    local_header_offset: u64,
    encrypted: bool,
    sha256: Option<[u8; 32]>,
    zip64_block: bool,
}

/// Parses `count` Entries from the central directory bytes, which start at file offset `base`.
pub(crate) fn parse_entries(bytes: &[u8], base: u64, count: u64) -> Result<Directory, FormatError> {
    let mut reader = ByteReader::with_base(bytes, base);
    let capacity = usize::try_from(count)
        .unwrap_or(0)
        .min(bytes.len() / CENTRAL_HEADER_LEN);
    let mut entries = Vec::with_capacity(capacity);
    let mut log = IssueLog::default();
    for _ in 0..count {
        let entry = parse_entry(&mut reader, entries.len(), &mut log)?;
        entries.push(entry);
    }
    Ok(Directory {
        entries,
        log,
        installation_flags: None,
    })
}

/// One central-directory header read on its own.
pub(crate) struct ParsedHeader<'a> {
    pub(crate) entry: Entry,
    pub(crate) name: &'a [u8],
    pub(crate) len: usize,
    pub(crate) zip64_block: bool,
}

/// Parses the single header at the start of `bytes`, which lies at file offset `base`, as the
/// Entry at `index`.
pub(crate) fn parse_header<'a>(
    bytes: &'a [u8],
    base: u64,
    index: usize,
    log: &mut IssueLog,
) -> Result<ParsedHeader<'a>, FormatError> {
    let mut reader = ByteReader::with_base(bytes, base);
    let (entry, name, zip64_block) = parse_entry_parts(&mut reader, index, log)?;
    let len = bytes.len() - reader.remaining();
    Ok(ParsedHeader {
        entry,
        name,
        len,
        zip64_block,
    })
}

fn parse_entry(
    reader: &mut ByteReader<'_>,
    index: usize,
    log: &mut IssueLog,
) -> Result<Entry, FormatError> {
    parse_entry_parts(reader, index, log).map(|(entry, _, _)| entry)
}

fn parse_entry_parts<'a>(
    reader: &mut ByteReader<'a>,
    index: usize,
    log: &mut IssueLog,
) -> Result<(Entry, &'a [u8], bool), FormatError> {
    let offset = reader.offset();
    let fixed = read_fixed_fields(reader, offset)?;
    let name = reader.read_bytes(fixed.name_len)?;
    let path = decode_path(name, index, log).into_owned();
    let extra_offset = reader.offset();
    let extra = reader.read_bytes(fixed.extra_len)?;
    reader.skip(fixed.comment_len)?;
    let extra = parse_extra(extra, extra_offset, &fixed, index, log)?;
    let zip64_block = extra.zip64_block;
    let entry = Entry {
        path,
        compressed_size: extra.compressed_size,
        uncompressed_size: extra.uncompressed_size,
        checksum: Checksum::Crc32c(fixed.crc32c),
        sha256: extra.sha256,
        sha256_checked: true,
        data_offset: extra.local_header_offset,
        compression: Compression::from_method(fixed.method),
        index: u32::try_from(index).unwrap_or(u32::MAX),
        encrypted: extra.encrypted,
        local_header: true,
        installed_bytes: None,
    };
    Ok((entry, name, zip64_block))
}

fn read_fixed_fields(reader: &mut ByteReader<'_>, offset: u64) -> Result<FixedFields, FormatError> {
    if reader.read_u32()? != CENTRAL_HEADER_SIGNATURE {
        return Err(FormatError::corrupt(
            offset,
            "central directory header signature missing",
        ));
    }
    reader.skip(VERSIONS_AND_FLAGS_LEN)?;
    let method = reader.read_u16()?;
    reader.skip(TIME_AND_DATE_LEN)?;
    let crc32c = reader.read_u32()?;
    let compressed_size = reader.read_u32()?;
    let uncompressed_size = reader.read_u32()?;
    let name_len = usize::from(reader.read_u16()?);
    let extra_len = usize::from(reader.read_u16()?);
    let comment_len = usize::from(reader.read_u16()?);
    reader.skip(DISK_AND_ATTRIBUTES_LEN)?;
    let local_header_offset = reader.read_u32()?;
    Ok(FixedFields {
        method,
        crc32c,
        compressed_size,
        uncompressed_size,
        local_header_offset,
        name_len,
        extra_len,
        comment_len,
    })
}

fn parse_extra(
    extra: &[u8],
    base: u64,
    fixed: &FixedFields,
    index: usize,
    log: &mut IssueLog,
) -> Result<ExtraFields, FormatError> {
    let mut fields = ExtraFields {
        compressed_size: u64::from(fixed.compressed_size),
        uncompressed_size: u64::from(fixed.uncompressed_size),
        local_header_offset: u64::from(fixed.local_header_offset),
        encrypted: false,
        sha256: None,
        zip64_block: false,
    };
    let mut reader = ByteReader::with_base(extra, base);
    while reader.remaining() >= BLOCK_HEADER_LEN {
        let block_offset = reader.offset();
        let id = reader.read_u16()?;
        let stored_len = reader.read_u16()?;
        let len = usize::from(stored_len);
        let data_len = len.checked_sub(BLOCK_HEADER_LEN).ok_or_else(|| {
            FormatError::corrupt(
                block_offset,
                format!("extra block 0x{id:04X} is shorter than its header"),
            )
        })?;
        let data = reader.read_bytes(data_len)?;
        let mut block = ByteReader::with_base(data, block_offset + BLOCK_HEADER_LEN as u64);
        match id {
            BLOCK_ZIP64 => {
                fields.zip64_block = true;
                read_zip64(&mut block, fixed, &mut fields)?;
            }
            BLOCK_CIG_ENCRYPTION => {
                fields.encrypted = read_encryption(&mut block, stored_len, index, log);
            }
            BLOCK_CIG_HASH => {
                fields.sha256 = read_hash(data, stored_len, index, log);
            }
            _ => {}
        }
    }
    if reader.remaining() > 0 {
        return Err(FormatError::corrupt(
            reader.offset(),
            "extra field ends inside a block header",
        ));
    }
    Ok(fields)
}

/// Reads the CIG hash block, whose data is the 32-byte SHA-256. A block of another length is
/// recorded in `log` and states none. When an Entry carries several, the last one decides.
fn read_hash(data: &[u8], stored_len: u16, index: usize, log: &mut IssueLog) -> Option<[u8; 32]> {
    let digest = stated_sha256(data);
    if digest.is_none() && data.len() != SHA256_LEN {
        log.record(index, EntryIssue::HashBlockSize { size: stored_len });
    }
    digest
}

/// Reads the CIG encryption block: 0 is plain and 1 encrypted. Any other value and a block of
/// another length than 6 bytes are recorded in `log`; a value that is not 0 or 1, and a block
/// too short to hold one, count as encrypted.
fn read_encryption(
    block: &mut ByteReader<'_>,
    stored_len: u16,
    index: usize,
    log: &mut IssueLog,
) -> bool {
    match (stored_len, block.read_u16()) {
        (ENCRYPTION_BLOCK_LEN, Ok(0)) => false,
        (ENCRYPTION_BLOCK_LEN, Ok(1)) => true,
        (len, value) => encryption_defect(len, value.ok(), index, log),
    }
}

#[cold]
#[inline(never)]
fn encryption_defect(len: u16, value: Option<u16>, index: usize, log: &mut IssueLog) -> bool {
    if len != ENCRYPTION_BLOCK_LEN {
        log.record(index, EntryIssue::EncryptionBlockSize { size: len });
    }
    match value {
        Some(0) => false,
        Some(1) | None => true,
        Some(value) => {
            log.record(index, EntryIssue::UnknownEncryption { value });
            true
        }
    }
}

fn read_zip64(
    block: &mut ByteReader<'_>,
    fixed: &FixedFields,
    fields: &mut ExtraFields,
) -> Result<(), ParseError> {
    if fixed.uncompressed_size == ZIP64_MARKER_32 {
        fields.uncompressed_size = block.read_u64()?;
    }
    if fixed.compressed_size == ZIP64_MARKER_32 {
        fields.compressed_size = block.read_u64()?;
    }
    if fixed.local_header_offset == ZIP64_MARKER_32 {
        fields.local_header_offset = block.read_u64()?;
    }
    Ok(())
}
