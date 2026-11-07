// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::{ByteReader, ParseError};

use crate::archive_metadata::ArchiveMetadata;
use crate::entry::stated_sha256;
use crate::extra_block_header::ExtraBlockHeader;
use crate::format_error::FormatError;
use crate::issue::{Directory, IssueLog, decode_path};
use crate::metadata_range::MetadataRange;
use crate::{
    Checksum, Compression, DosTimestamp, Entry, EntryIssue, EntryMetadata, ZipHeaderMetadata,
};

const CENTRAL_HEADER_SIGNATURE: u32 = 0x0201_4B50;
pub(crate) const CENTRAL_HEADER_LEN: usize = 46;
const ZIP64_MARKER_32: u32 = 0xFFFF_FFFF;
const BLOCK_ZIP64: u16 = 0x0001;
const BLOCK_CIG_ENCRYPTION: u16 = 0x5002;
const BLOCK_CIG_HASH: u16 = 0x5003;
const BLOCK_HEADER_LEN: usize = 4;
const ENCRYPTION_BLOCK_LEN: u16 = 6;
const SHA256_LEN: usize = 32;

struct FixedFields {
    dos_timestamp: DosTimestamp,
    header: ZipHeaderMetadata,
    method: u16,
    crc32c: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    local_header_offset: u32,
    name_len: usize,
    extra_len: usize,
    comment_len: usize,
}

#[derive(Clone, Copy)]
struct RecordPosition {
    index: usize,
    original_directory_ordinal: Option<u64>,
    source_len: u64,
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
pub(crate) fn parse_entries(
    bytes: &[u8],
    base: u64,
    count: u64,
    metadata: ArchiveMetadata,
) -> Result<Directory, FormatError> {
    let mut reader = ByteReader::with_base(bytes, base);
    let capacity = usize::try_from(count)
        .unwrap_or(0)
        .min(bytes.len() / CENTRAL_HEADER_LEN);
    let mut entries = Vec::with_capacity(capacity);
    let mut log = IssueLog::default();
    for ordinal in 0..count {
        let position = RecordPosition {
            index: entries.len(),
            original_directory_ordinal: Some(ordinal),
            source_len: metadata.source_len(),
        };
        let entry = parse_entry(&mut reader, position, &mut log)?;
        entries.push(entry);
    }
    Ok(Directory {
        entries,
        log,
        installation_flags: None,
        metadata,
    })
}

/// One central-directory header read on its own.
pub(crate) struct ParsedHeader<'a> {
    pub(crate) entry: Entry,
    pub(crate) name: &'a [u8],
    pub(crate) len: usize,
    pub(crate) zip64_block: bool,
}

/// Parses the standalone header at `base` as Entry `index`, without an original directory ordinal.
pub(crate) fn parse_header<'a>(
    bytes: &'a [u8],
    base: u64,
    index: usize,
    log: &mut IssueLog,
    source_len: u64,
) -> Result<ParsedHeader<'a>, FormatError> {
    let mut reader = ByteReader::with_base(bytes, base);
    let position = RecordPosition {
        index,
        original_directory_ordinal: None,
        source_len,
    };
    let (entry, name, zip64_block) = parse_entry_parts(&mut reader, position, log)?;
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
    position: RecordPosition,
    log: &mut IssueLog,
) -> Result<Entry, FormatError> {
    parse_entry_parts(reader, position, log).map(|(entry, _, _)| entry)
}

fn parse_entry_parts<'a>(
    reader: &mut ByteReader<'a>,
    position: RecordPosition,
    log: &mut IssueLog,
) -> Result<(Entry, &'a [u8], bool), FormatError> {
    let RecordPosition {
        index,
        original_directory_ordinal,
        source_len,
    } = position;
    let offset = reader.offset();
    let fixed = read_fixed_fields(reader, offset)?;
    let name = reader.read_bytes(fixed.name_len)?;
    let path = decode_path(name, index, log).into_owned();
    let extra_offset = reader.offset();
    let extra = reader.read_bytes(fixed.extra_len)?;
    reader.skip(fixed.comment_len)?;
    let extra = parse_extra(extra, extra_offset, &fixed, index, log)?;
    let record_len = reader
        .offset()
        .checked_sub(offset)
        .ok_or_else(|| FormatError::corrupt(offset, "directory-record range underflows"))?;
    let record = MetadataRange::checked(offset, record_len, source_len)?;
    let zip64_block = extra.zip64_block;
    let entry = Entry {
        metadata: EntryMetadata::from_zip(
            fixed.dos_timestamp,
            fixed.method,
            original_directory_ordinal,
            fixed.header,
            record,
        ),
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
    let version_made_by = reader.read_u16()?;
    let version_needed = reader.read_u16()?;
    let general_purpose_flags = reader.read_u16()?;
    let method = reader.read_u16()?;
    let time = reader.read_u16()?;
    let date = reader.read_u16()?;
    let crc32c = reader.read_u32()?;
    let compressed_size = reader.read_u32()?;
    let uncompressed_size = reader.read_u32()?;
    let name_len = usize::from(reader.read_u16()?);
    let extra_len = usize::from(reader.read_u16()?);
    let comment_len = usize::from(reader.read_u16()?);
    let disk_start = reader.read_u16()?;
    let internal_attributes = reader.read_u16()?;
    let external_attributes = reader.read_u32()?;
    let local_header_offset = reader.read_u32()?;
    Ok(FixedFields {
        dos_timestamp: DosTimestamp::new(date, time),
        header: ZipHeaderMetadata::new(
            version_made_by,
            version_needed,
            general_purpose_flags,
            disk_start,
            internal_attributes,
            external_attributes,
        ),
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

pub(crate) struct RecordLengths {
    pub(crate) name: usize,
    pub(crate) extra: usize,
    pub(crate) comment: usize,
}

pub(crate) fn record_lengths(bytes: &[u8], offset: u64) -> Result<RecordLengths, FormatError> {
    let fixed = read_fixed_fields(&mut ByteReader::with_base(bytes, offset), offset)?;
    Ok(RecordLengths {
        name: fixed.name_len,
        extra: fixed.extra_len,
        comment: fixed.comment_len,
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
        let header = ExtraBlockHeader::read(&mut reader)?;
        let id = header.id();
        let stored_len = header.declared_len();
        let data_len = header.body_len().ok_or_else(|| {
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
