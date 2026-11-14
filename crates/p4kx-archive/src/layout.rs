// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::archive_metadata::ArchiveMetadata;
use crate::format_error::FormatError;
use crate::issue::Directory;
use crate::source::Source;
use crate::{Checksum, central_directory, directory_v2, end_records, recovery};

const LAYOUT_MARKER_LEN: usize = 6;
const LAYOUT_MAGIC: [u8; 4] = *b"Jiji";
const LAYOUT_V2: u16 = 2;

/// Reads the Entries of the Archive in whichever layout it is written, refusing more than
/// `max_entries` before the central directory is read.
///
/// An Archive whose last bytes are a layout version followed by `Jiji` is read in that layout;
/// any other file is read as a v1 ZIP64 Archive, or as a plain ZIP archive when its end record
/// has no ZIP64 locator, whose Entries carry the plain ZIP CRC-32.
pub(crate) fn read_entries(
    source: &Source,
    max_entries: Option<u64>,
) -> Result<Directory, FormatError> {
    match layout_version(source)? {
        None => read_v1_entries(source, max_entries),
        Some(LAYOUT_V2) => directory_v2::read_entries(source, max_entries),
        Some(version) => Err(FormatError::UnsupportedLayout { version }),
    }
}

/// Reads the Entries like [`read_entries`] and, when the v1 or plain ZIP directory cannot be
/// read, rebuilds them by scanning for central-directory records. An Archive in the v2 layout, an
/// unreadable file and a scan that finds no record keep their original error.
pub(crate) fn read_entries_recovering(source: &Source) -> Result<Directory, FormatError> {
    match read_entries(source, None) {
        Err(
            error @ (FormatError::NotAnArchive
            | FormatError::Corrupt { .. }
            | FormatError::Truncated(_)),
        ) if layout_version(source)?.is_none() => {
            let hint = end_records::locate_directory(source)
                .ok()
                .map(|location| location.offset);
            recovery::recover(source, hint)?.ok_or(error)
        }
        other => other,
    }
}

/// Refuses an Entry count above `max_entries`.
pub(crate) fn check_entry_count(count: u64, max_entries: Option<u64>) -> Result<(), FormatError> {
    match max_entries {
        Some(max) if count > max => Err(FormatError::LimitExceeded(format!(
            "it holds {count} Entries, more than {max}"
        ))),
        _ => Ok(()),
    }
}

fn layout_version(source: &Source) -> Result<Option<u16>, FormatError> {
    let Some(offset) = source.len().checked_sub(LAYOUT_MARKER_LEN as u64) else {
        return Ok(None);
    };
    let marker = source.read_at(offset, LAYOUT_MARKER_LEN)?;
    match marker.split_first_chunk::<2>() {
        Some((version, magic)) if magic == LAYOUT_MAGIC => Ok(Some(u16::from_le_bytes(*version))),
        _ => Ok(None),
    }
}

fn read_v1_entries(source: &Source, max_entries: Option<u64>) -> Result<Directory, FormatError> {
    let location = end_records::locate_directory(source)?;
    check_entry_count(location.entry_count, max_entries)?;
    let size = usize::try_from(location.size).map_err(|_| {
        FormatError::corrupt(
            location.offset,
            "central directory does not fit into memory",
        )
    })?;
    let directory = source.read_at(location.offset, size)?;
    let mut directory = central_directory::parse_entries(
        &directory,
        location.offset,
        location.entry_count,
        ArchiveMetadata::zip(source.len(), &location.metadata),
    )?;
    if !location.zip64 {
        for entry in &mut directory.entries {
            if let Some(value) = entry.crc32c() {
                entry.checksum = Checksum::Crc32(value);
            }
        }
    }
    Ok(directory)
}
