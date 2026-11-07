// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::ByteReader;

use crate::directory_v2_trailer::{RECORD_LEN, TRAILER_LEN, Trailer};
use crate::entry::stated_sha256;
use crate::format_error::FormatError;
use crate::issue::{Directory, IssueLog, decode_path};
use crate::source::Source;
use crate::{Checksum, Compression, DosTimestamp, Entry, EntryIssue, EntryMetadata};

const RECORD_SIGNATURE_LEN: usize = 128;
const RECORD_SHA256_LEN: usize = 32;

/// Reads the Entries of a v2 Archive from its trailer, directory records and name table, refusing
/// more than `max_entries` before the directory is read.
pub(crate) fn read_entries(
    source: &Source,
    max_entries: Option<u64>,
) -> Result<Directory, FormatError> {
    let trailer_offset = source
        .len()
        .checked_sub(TRAILER_LEN as u64)
        .ok_or_else(|| FormatError::corrupt(0, "v2 trailer is cut off"))?;
    let trailer = Trailer::read(source, trailer_offset, max_entries)?;
    let directory = read_range(
        source,
        trailer.directory_offset,
        trailer.directory_size()?,
        trailer_offset,
        "central directory",
    )?;
    let names = read_range(
        source,
        trailer.names_offset,
        trailer.names_size()?,
        trailer_offset,
        "name table",
    )?;
    let mut entries = Vec::with_capacity(directory.len() / RECORD_LEN);
    let mut log = IssueLog::default();
    let data_limit = trailer.payload_end;
    let progress = trailer.progress(source)?;
    let mut installed = ByteReader::with_base(&progress, trailer.payload_end);
    let mut record_offset = trailer.directory_offset;
    for record in directory.as_chunks::<RECORD_LEN>().0 {
        let mut entry = parse_record(
            record,
            record_offset,
            &names,
            (entries.len(), data_limit),
            &mut log,
        )?;
        let installed_bytes = installed.read_u64()?;
        if installed_bytes > entry.compressed_size {
            return Err(FormatError::corrupt(
                record_offset,
                "installed-byte counter exceeds stored size",
            ));
        }
        entry.installed_bytes = Some(installed_bytes);
        entries.push(entry);
        record_offset += RECORD_LEN as u64;
    }
    Ok(Directory {
        entries,
        log,
        installation_flags: Some(trailer.flags),
    })
}

fn read_range(
    source: &Source,
    offset: u64,
    size: u64,
    trailer_offset: u64,
    what: &str,
) -> Result<Vec<u8>, FormatError> {
    if offset
        .checked_add(size)
        .is_none_or(|end| end > trailer_offset)
    {
        return Err(FormatError::corrupt(
            trailer_offset,
            format!("{what} lies outside the file"),
        ));
    }
    let size = usize::try_from(size)
        .map_err(|_| FormatError::corrupt(offset, format!("{what} does not fit into memory")))?;
    Ok(source.read_at(offset, size)?)
}

/// Parses the record at file `offset`; `at` is the Entry's position and the offset of the central
/// directory or the name table, whichever comes first, which its data must end before.
fn parse_record(
    record: &[u8],
    offset: u64,
    names: &[u8],
    at: (usize, u64),
    log: &mut IssueLog,
) -> Result<Entry, FormatError> {
    let (index, data_limit) = at;
    let mut reader = ByteReader::with_base(record, offset);
    let method = reader.read_u16()?;
    let time = reader.read_u16()?;
    let date = reader.read_u16()?;
    let crc32c = reader.read_u32()?;
    let compressed_size = reader.read_u64()?;
    let uncompressed_size = reader.read_u64()?;
    let data_offset = reader.read_u64()?;
    let name_offset = reader.read_u64()?;
    reader.skip(RECORD_SIGNATURE_LEN)?;
    let encryption = reader.read_u16()?;
    let sha256 = stated_sha256(reader.read_bytes(RECORD_SHA256_LEN)?);
    let ordinal = u64::try_from(index)
        .map_err(|_| FormatError::corrupt(offset, "v2 record ordinal exceeds u64"))?;
    if data_offset
        .checked_add(compressed_size)
        .is_none_or(|end| end > data_limit)
        || encryption > 1
    {
        record_defects(
            log,
            index,
            (data_offset, compressed_size, data_limit),
            encryption,
        );
    }
    Ok(Entry {
        metadata: EntryMetadata::from_v2(
            DosTimestamp::new(date, time),
            method,
            ordinal,
            encryption,
        ),
        path: read_name(names, name_offset, offset, index, log)?,
        compressed_size,
        uncompressed_size,
        checksum: Checksum::Crc32c(crc32c),
        sha256,
        sha256_checked: false,
        data_offset,
        compression: Compression::from_method(method),
        index: u32::try_from(index).unwrap_or(u32::MAX),
        encrypted: encryption != 0,
        local_header: false,
        installed_bytes: None,
    })
}

#[cold]
#[inline(never)]
fn record_defects(log: &mut IssueLog, index: usize, data: (u64, u64, u64), encryption: u16) {
    let (data_offset, compressed_size, data_limit) = data;
    if data_offset
        .checked_add(compressed_size)
        .is_none_or(|end| end > data_limit)
    {
        log.record(index, EntryIssue::DataOutsideFile);
    }
    if encryption > 1 {
        log.record(index, EntryIssue::UnknownEncryption { value: encryption });
    }
}

fn read_name(
    names: &[u8],
    name_offset: u64,
    record_offset: u64,
    index: usize,
    log: &mut IssueLog,
) -> Result<String, FormatError> {
    let name = usize::try_from(name_offset)
        .ok()
        .and_then(|start| names.get(start..))
        .ok_or_else(|| {
            FormatError::corrupt(record_offset, "Entry path lies outside the name table")
        })?;
    let end = name
        .iter()
        .position(|&byte| byte == 0)
        .ok_or_else(|| FormatError::corrupt(record_offset, "Entry path is not terminated"))?;
    let name = name.get(..end).unwrap_or_default();
    if name.is_empty() {
        return Err(FormatError::corrupt(record_offset, "Entry path is empty"));
    }
    let path = decode_path(name, index, log);
    Ok(if path.contains('/') {
        path.replace('/', "\\")
    } else {
        path.into_owned()
    })
}
