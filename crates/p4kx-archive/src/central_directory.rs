// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::{ByteReader, ParseError};

use crate::archive_metadata::ArchiveMetadata;
use crate::encryption_declarations::{
    ENCRYPTION_BLOCK_LEN, EncryptionDeclarationsBuilder, StoredEncryptionDeclaration,
};
use crate::entry::stated_sha256;
use crate::extra_block_header::ExtraBlockHeader;
use crate::format_error::FormatError;
use crate::issue::{Directory, IssueLog, decode_path};
use crate::metadata_range::MetadataRange;
use crate::recovery_data_outside_file::{Zip64BoundOffsets, ZipBoundWords};
use crate::{
    Checksum, Compression, DosTimestamp, Entry, EntryIssue, EntryMetadata, ObservedHashBlockSize,
    ZipHeaderMetadata,
};

pub(crate) const CENTRAL_HEADER_SIGNATURE: u32 = 0x0201_4B50;
pub(crate) const CENTRAL_HEADER_LEN: usize = 46;
pub(crate) const MAX_HEADER_LEN: usize = CENTRAL_HEADER_LEN + 3 * u16::MAX as usize;
const ZIP64_MARKER_32: u32 = 0xFFFF_FFFF;
const BLOCK_ZIP64: u16 = 0x0001;
const BLOCK_CIG_ENCRYPTION: u16 = 0x5002;
const BLOCK_CIG_HASH: u16 = 0x5003;
const BLOCK_HEADER_LEN: usize = 4;
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
    encryption: EncryptionDeclarationsBuilder,
    sha256: Option<[u8; 32]>,
    zip64_block: bool,
    zip64_offsets: Zip64BoundOffsets,
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
        let count_before = log.count;
        let entry = parse_entry(&mut reader, position, &mut log)?;
        log.record_empty_folder_issues(&entry, count_before);
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
    pub(crate) bounds: ZipBoundWords,
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
    parse_entry_parts(&mut reader, position, log)
}

fn parse_entry(
    reader: &mut ByteReader<'_>,
    position: RecordPosition,
    log: &mut IssueLog,
) -> Result<Entry, FormatError> {
    parse_entry_parts(reader, position, log).map(|header| header.entry)
}

fn parse_entry_parts<'a>(
    reader: &mut ByteReader<'a>,
    position: RecordPosition,
    log: &mut IssueLog,
) -> Result<ParsedHeader<'a>, FormatError> {
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
    let extra_len = extra.len() as u64;
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
            extra.encryption.finish(),
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
    Ok(ParsedHeader {
        entry,
        name,
        len: CENTRAL_HEADER_LEN + fixed.name_len + fixed.extra_len + fixed.comment_len,
        zip64_block,
        bounds: ZipBoundWords {
            offsets: extra.zip64_offsets,
            extra_len,
            has_size_marker: fixed.compressed_size == ZIP64_MARKER_32,
            has_offset_marker: fixed.local_header_offset == ZIP64_MARKER_32,
        },
    })
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
        encryption: EncryptionDeclarationsBuilder::default(),
        sha256: None,
        zip64_block: false,
        zip64_offsets: Zip64BoundOffsets::default(),
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
                read_zip64(&mut block, block_offset, fixed, &mut fields)?;
            }
            BLOCK_CIG_ENCRYPTION => {
                fields.encrypted = read_encryption(
                    &mut block,
                    stored_len,
                    data.first().copied(),
                    &mut fields.encryption,
                    index,
                    log,
                );
            }
            BLOCK_CIG_HASH => {
                fields.sha256 =
                    read_hash(data, block_offset, block.offset(), stored_len, index, log)?;
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

fn read_hash(
    data: &[u8],
    start: u64,
    end: u64,
    stored_len: u16,
    index: usize,
    log: &mut IssueLog,
) -> Result<Option<[u8; 32]>, FormatError> {
    if data.len() != SHA256_LEN {
        log.record_hash_block_size(index, stored_len, ObservedHashBlockSize::at(start, end)?);
    }
    Ok(stated_sha256(data))
}

/// Reads the CIG encryption block: 0 is plain and 1 encrypted. Any other value and a block of
/// another length than 6 bytes are recorded in `log`; a value that is not 0 or 1, and a block
/// too short to hold one, count as encrypted.
fn read_encryption(
    block: &mut ByteReader<'_>,
    stored_len: u16,
    first_byte: Option<u8>,
    declarations: &mut EncryptionDeclarationsBuilder,
    index: usize,
    log: &mut IssueLog,
) -> bool {
    let value_offset = block.offset();
    let value = block.read_u16();
    declarations.push(StoredEncryptionDeclaration::cig(
        value_offset,
        stored_len,
        value.as_ref().ok().copied(),
        first_byte,
    ));
    match (stored_len, value) {
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
    block_offset: u64,
    fixed: &FixedFields,
    fields: &mut ExtraFields,
) -> Result<(), ParseError> {
    if fixed.uncompressed_size == ZIP64_MARKER_32 {
        fields.uncompressed_size = block.read_u64()?;
    }
    if fixed.compressed_size == ZIP64_MARKER_32 {
        let at = block.offset();
        fields.compressed_size = block.read_u64()?;
        fields.zip64_offsets.compressed_size = Some(at);
        fields.zip64_offsets.header = Some(block_offset);
    }
    if fixed.local_header_offset == ZIP64_MARKER_32 {
        let at = block.offset();
        fields.local_header_offset = block.read_u64()?;
        fields.zip64_offsets.local_header_offset = Some(at);
        fields.zip64_offsets.header = Some(block_offset);
    }
    Ok(())
}
