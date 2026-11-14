// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::cig_install_metadata::CigInstallMetadata;
use crate::format_error::FormatError;
use crate::metadata_range::MetadataRange;
use crate::source::Source;
use crate::zip_directory_metadata::ZipDirectoryMetadata;
use crate::zip_end_metadata::ZipEndMetadata;
use crate::zip64_end_metadata::Zip64EndMetadata;
use crate::zip64_locator_metadata::Zip64LocatorMetadata;

const END_SIGNATURE: u32 = 0x0605_4b50;
const ZIP64_LOCATOR_SIGNATURE: u32 = 0x0706_4b50;
const ZIP64_END_SIGNATURE: u32 = 0x0606_4b50;
const END_LEN: usize = 22;
const ZIP64_LOCATOR_LEN: usize = 20;
const ZIP64_END_LEN: usize = 56;

/// The selected central directory and the source facts used to locate it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DirectoryLocation {
    pub(crate) entry_count: u64,
    pub(crate) offset: u64,
    pub(crate) size: u64,
    pub(crate) zip64: bool,
    pub(crate) metadata: ZipDirectoryMetadata,
}

/// Reads the existing ZIP end/locator route while retaining its original words and extents.
pub(crate) fn locate_directory(source: &Source) -> Result<DirectoryLocation, FormatError> {
    let tail_len = source.len().min((END_LEN + usize::from(u16::MAX)) as u64);
    let tail_start = source.len() - tail_len;
    let tail = source.read_at(tail_start, usize::try_from(tail_len).unwrap_or(usize::MAX))?;
    let end_index = find_end_record(&tail).ok_or(FormatError::NotAnArchive)?;
    let end_offset = tail_start + end_index as u64;
    let bytes = tail.get(end_index..).unwrap_or_default();
    let end = ZipEndMetadata::read(bytes, end_offset, source.len())?;
    let locator = find_zip64_locator(source, &tail, end_index, tail_start, &end)?;
    let zip64 = locator
        .map(|value| read_zip64_end(source, value.target()))
        .transpose()?;
    let (entry_count, offset, size) = match zip64 {
        Some(value) => (
            value.entries(),
            value.directory_offset(),
            value.directory_size(),
        ),
        None => plain_location(end, end_offset)?,
    };
    let record_offset = zip64.map_or(end_offset, |value| value.fixed().offset());
    check_before(offset, size, record_offset)?;
    let comment = bytes
        .get(END_LEN..)
        .and_then(|bytes| bytes.get(..usize::from(end.comment_len())));
    let installation = if zip64.is_some() {
        match comment {
            Some(bytes) => CigInstallMetadata::read(bytes, entry_count, offset, source.len())?,
            None => None,
        }
    } else {
        None
    };
    let metadata = ZipDirectoryMetadata {
        records: MetadataRange::checked(offset, size, source.len())?,
        count: entry_count,
        end,
        zip64,
        locator,
        installation,
    };
    Ok(DirectoryLocation {
        entry_count,
        offset,
        size,
        zip64: zip64.is_some(),
        metadata,
    })
}

fn find_zip64_locator(
    source: &Source,
    tail: &[u8],
    end_index: usize,
    tail_start: u64,
    end: &ZipEndMetadata,
) -> Result<Option<Zip64LocatorMetadata>, FormatError> {
    let end_offset = tail_start + end_index as u64;
    if let Some(offset) = end_offset.checked_sub(ZIP64_LOCATOR_LEN as u64) {
        let bytes = source.read_at(offset, ZIP64_LOCATOR_LEN)?;
        if let Some(locator) = locator_target(&bytes, 0, offset, source.len())?
            && points_at_zip64_end(source, locator.target(), offset)?
        {
            return Ok(Some(locator));
        }
    }
    if !has_zip64_markers(end) {
        return Ok(None);
    }
    let Some(adjacent) = end_index.checked_sub(ZIP64_LOCATOR_LEN) else {
        return Ok(None);
    };
    let signature = ZIP64_LOCATOR_SIGNATURE.to_le_bytes();
    for start in (0..adjacent).rev() {
        if tail.get(start..start + signature.len()) != Some(signature.as_slice()) {
            continue;
        }
        let Some(locator) = locator_target(tail, start, tail_start, source.len())? else {
            continue;
        };
        if points_at_zip64_end(source, locator.target(), tail_start + start as u64)? {
            return Ok(Some(locator));
        }
    }
    Ok(None)
}

fn points_at_zip64_end(
    source: &Source,
    target: u64,
    locator_offset: u64,
) -> Result<bool, FormatError> {
    Ok(target < locator_offset && has_zip64_end_signature(source, target)?)
}

const fn has_zip64_markers(end: &ZipEndMetadata) -> bool {
    end.entries() == u16::MAX
        || end.directory_size() == u32::MAX
        || end.directory_offset() == u32::MAX
}

fn locator_target(
    tail: &[u8],
    start: usize,
    tail_start: u64,
    source_len: u64,
) -> Result<Option<Zip64LocatorMetadata>, FormatError> {
    let Some(bytes) = tail.get(start..start + ZIP64_LOCATOR_LEN) else {
        return Ok(None);
    };
    if bytes.get(..4) != Some(ZIP64_LOCATOR_SIGNATURE.to_le_bytes().as_slice()) {
        return Ok(None);
    }
    Ok(Some(Zip64LocatorMetadata::read(
        bytes,
        tail_start + start as u64,
        source_len,
    )?))
}

fn has_zip64_end_signature(source: &Source, offset: u64) -> Result<bool, FormatError> {
    let signature = ZIP64_END_SIGNATURE.to_le_bytes();
    if offset
        .checked_add(signature.len() as u64)
        .is_none_or(|end| end > source.len())
    {
        return Ok(false);
    }
    Ok(source.read_at(offset, signature.len())? == signature)
}

fn plain_location(end: ZipEndMetadata, offset: u64) -> Result<(u64, u64, u64), FormatError> {
    if has_zip64_markers(&end) {
        return Err(FormatError::corrupt(
            offset,
            "no ZIP64 locator before the end record",
        ));
    }
    Ok((
        u64::from(end.entries()),
        u64::from(end.directory_offset()),
        u64::from(end.directory_size()),
    ))
}

fn read_zip64_end(source: &Source, offset: u64) -> Result<Zip64EndMetadata, FormatError> {
    if offset
        .checked_add(ZIP64_END_LEN as u64)
        .is_none_or(|end| end > source.len())
    {
        return Err(FormatError::corrupt(
            offset,
            "ZIP64 end record lies outside the file",
        ));
    }
    Zip64EndMetadata::read(
        &source.read_at(offset, ZIP64_END_LEN)?,
        offset,
        source.len(),
    )
}

fn check_before(offset: u64, size: u64, record_offset: u64) -> Result<(), FormatError> {
    if offset
        .checked_add(size)
        .is_none_or(|end| end > record_offset)
    {
        return Err(FormatError::corrupt(
            record_offset,
            "central directory lies outside the file",
        ));
    }
    Ok(())
}

fn find_end_record(tail: &[u8]) -> Option<usize> {
    let signature = END_SIGNATURE.to_le_bytes();
    let last_start = tail.len().checked_sub(END_LEN)?;
    (0..=last_start)
        .rev()
        .find(|&index| tail.get(index..index + signature.len()) == Some(signature.as_slice()))
}
