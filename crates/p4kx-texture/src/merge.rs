// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Write};

use crate::header::{CRYENGINE_TRAILER, MAX_HEADER_LEN};
use crate::{DdsHeader, TextureError};

const COPY_BUFFER_LEN: usize = 64 * 1024;
const INITIAL_CAPACITY: u64 = 16 * 1024 * 1024;

/// Gives access to the sibling parts `.dds.1` … `.dds.N` of a header Entry, wherever they are
/// stored.
pub trait SiblingReader {
    /// The reader of one part's data.
    type Part<'a>: Read
    where
        Self: 'a;

    /// Returns the length in bytes of part `index`, or `None` when the part does not exist.
    ///
    /// # Errors
    ///
    /// Returns the I/O error of looking the part up.
    fn part_len(&self, index: u32) -> io::Result<Option<u64>>;

    /// Opens the data of part `index`.
    ///
    /// # Errors
    ///
    /// Returns the I/O error of opening the part.
    fn open_part(&self, index: u32) -> io::Result<Self::Part<'_>>;
}

/// What a merge wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergeReport {
    /// The header of the merged texture.
    pub header: DdsHeader,
    /// Number of sibling parts read.
    pub parts: u32,
    /// Size of the merged DDS in bytes.
    pub size: u64,
}

/// Writes the header Entry `header` of `header_size` bytes and its sibling parts as one standard
/// DDS file to `out`.
///
/// The header comes first, then every mip from the largest to the smallest, image by image for a
/// cubemap or texture array. Every part's length is checked before anything is written. The
/// parts are streamed; a texture with several images per mip holds the small mips of the header
/// Entry in memory and opens each part once per image. The 8-byte CryEngine trailer `CExtCEnd`
/// some header Entries carry after the last mip is verified and left out.
///
/// # Errors
///
/// Returns a [`TextureError`] when the header is not a DDS header or its format's size is
/// unknown, a part is missing, truncated or oversized, the header's tail is not a run of whole
/// mips, the trailer is not `CExtCEnd` ([`TextureError::UnknownTrailer`]), or reading or writing
/// fails.
pub fn merge<R, S, W>(
    mut header: R,
    header_size: u64,
    siblings: &S,
    out: &mut W,
) -> Result<MergeReport, TextureError>
where
    R: Read,
    S: SiblingReader + ?Sized,
    W: Write + ?Sized,
{
    let mut prefix = [0; MAX_HEADER_LEN];
    let limit =
        usize::try_from(header_size).map_or(MAX_HEADER_LEN, |size| size.min(MAX_HEADER_LEN));
    let prefix_len = read_prefix(&mut header, prefix.get_mut(..limit).unwrap_or_default())?;
    let start = prefix.get(..prefix_len).unwrap_or_default();
    let dds = DdsHeader::parse(start)?;
    let entry_size = header_size;
    let header_size = dds.data_len(entry_size);
    let trailer_len = entry_size.saturating_sub(header_size);
    let parts = dds.part_count(header_size)?;
    let sizes = part_sizes(&dds, parts, siblings)?;
    let data_end = usize::try_from(header_size).map_or(prefix_len, |size| size.min(prefix_len));
    let (own_header, own_tail) = start
        .get(..data_end)
        .unwrap_or(start)
        .split_at_checked(dds.header_len())
        .unwrap_or((start, &[]));
    let after_data = start.get(data_end..).unwrap_or_default();
    out.write_all(own_header).map_err(TextureError::Write)?;
    let mut buffer = vec![0; COPY_BUFFER_LEN];
    if parts > 0 && dds.surfaces() > 1 {
        let mut tail = own_tail.chain(after_data).chain(header);
        let data = read_tail(&dds, &mut tail, header_size)?;
        check_trailer(tail, trailer_len, header_size)?;
        write_surfaces(&dds, parts, &data, siblings, out, &mut buffer)?;
        return finish(dds, parts, header_size, out);
    }
    for (index, expected) in (1..=parts).rev().zip(sizes.into_iter().rev()) {
        let reader = siblings
            .open_part(index)
            .map_err(|source| TextureError::ReadPart { index, source })?;
        let copied = copy(reader, expected, out, &mut buffer).map_err(|error| {
            error.into_texture_error(|source| TextureError::ReadPart { index, source })
        })?;
        if copied < expected {
            return Err(TextureError::TruncatedPart {
                index,
                expected,
                actual: copied,
            });
        }
    }
    out.write_all(own_tail).map_err(TextureError::Write)?;
    let rest = header_size.saturating_sub(prefix_len as u64);
    let copied = copy(&mut header, rest, out, &mut buffer)
        .map_err(|error| error.into_texture_error(TextureError::ReadHeader))?;
    if copied < rest {
        return Err(TextureError::TruncatedHeader {
            len: prefix_len as u64 + copied,
            needed: header_size,
        });
    }
    check_trailer(after_data.chain(header), trailer_len, header_size)?;
    finish(dds, parts, header_size, out)
}

/// Checks that the `trailer_len` bytes after the data of `data_len` bytes are the CryEngine
/// trailer. Nothing is read when `trailer_len` is 0.
pub(crate) fn check_trailer(
    reader: impl Read,
    trailer_len: u64,
    data_len: u64,
) -> Result<(), TextureError> {
    if trailer_len == 0 {
        return Ok(());
    }
    let read = read_up_to(reader, trailer_len).map_err(TextureError::ReadHeader)?;
    let Ok(found) = <[u8; 8]>::try_from(read.as_slice()) else {
        return Err(TextureError::TruncatedHeader {
            len: data_len.saturating_add(read.len() as u64),
            needed: data_len.saturating_add(trailer_len),
        });
    };
    if &found == CRYENGINE_TRAILER {
        Ok(())
    } else {
        Err(TextureError::UnknownTrailer { found })
    }
}

fn finish<W: Write + ?Sized>(
    dds: DdsHeader,
    parts: u32,
    header_size: u64,
    out: &mut W,
) -> Result<MergeReport, TextureError> {
    out.flush().map_err(TextureError::Write)?;
    let merged_parts: u64 = (1..=parts)
        .filter_map(|index| dds.mip_size(parts - index))
        .sum();
    Ok(MergeReport {
        header: dds,
        parts,
        size: header_size.saturating_add(merged_parts),
    })
}

/// Reads the mips that follow the header in the header Entry, each image's smallest mips one
/// image after the other.
fn read_tail(
    dds: &DdsHeader,
    tail: &mut impl Read,
    header_size: u64,
) -> Result<Vec<u8>, TextureError> {
    let header_len = dds.header_len() as u64;
    let len = header_size.saturating_sub(header_len);
    let data = read_up_to(tail, len).map_err(TextureError::ReadHeader)?;
    if (data.len() as u64) < len {
        return Err(TextureError::TruncatedHeader {
            len: header_len + data.len() as u64,
            needed: header_size,
        });
    }
    Ok(data)
}

/// Writes a texture with several images per mip in the standard layout: image by image, each
/// with its mips from the largest to the smallest. A part holds one mip of every image, so it is
/// opened once per image and read from that image's offset.
fn write_surfaces<S, W>(
    dds: &DdsHeader,
    parts: u32,
    tail: &[u8],
    siblings: &S,
    out: &mut W,
    buffer: &mut [u8],
) -> Result<(), TextureError>
where
    S: SiblingReader + ?Sized,
    W: Write + ?Sized,
{
    let unsupported = || TextureError::UnsupportedFormat(dds.format());
    let pitch = dds
        .surface_chain_size(parts..dds.mip_count())
        .and_then(|pitch| usize::try_from(pitch).ok())
        .ok_or_else(unsupported)?;
    for surface in 0..dds.surfaces() {
        let own_mips = usize::try_from(surface)
            .ok()
            .and_then(|surface| pitch.checked_mul(surface))
            .and_then(|start| tail.get(start..start.checked_add(pitch)?))
            .ok_or(TextureError::TailMismatch {
                tail: tail.len() as u64,
            })?;
        for level in 0..parts {
            let index = parts - level;
            let size = dds.surface_size(level).ok_or_else(unsupported)?;
            let offset = size.checked_mul(surface).ok_or_else(unsupported)?;
            let read_error = |source| TextureError::ReadPart { index, source };
            let mut reader = siblings.open_part(index).map_err(read_error)?;
            let skipped =
                io::copy(&mut reader.by_ref().take(offset), &mut io::sink()).map_err(read_error)?;
            let copied = if skipped == offset {
                copy(&mut reader, size, out, buffer)
                    .map_err(|error| error.into_texture_error(read_error))?
            } else {
                0
            };
            if copied < size {
                return Err(TextureError::TruncatedPart {
                    index,
                    expected: size,
                    actual: copied,
                });
            }
        }
        out.write_all(own_mips).map_err(TextureError::Write)?;
    }
    Ok(())
}

fn part_sizes<S>(dds: &DdsHeader, parts: u32, siblings: &S) -> Result<Vec<u64>, TextureError>
where
    S: SiblingReader + ?Sized,
{
    (1..=parts)
        .map(|index| checked_part_len(dds, parts, index, siblings))
        .collect()
}

/// Returns the length of part `index` of a texture split into `parts` parts after checking that
/// it exists and holds exactly its mip.
pub(crate) fn checked_part_len<S>(
    dds: &DdsHeader,
    parts: u32,
    index: u32,
    siblings: &S,
) -> Result<u64, TextureError>
where
    S: SiblingReader + ?Sized,
{
    let expected = dds
        .mip_size(parts - index)
        .ok_or_else(|| TextureError::UnsupportedFormat(dds.format()))?;
    let actual = siblings
        .part_len(index)
        .map_err(|source| TextureError::ReadPart { index, source })?
        .ok_or(TextureError::MissingPart { index })?;
    match actual.cmp(&expected) {
        std::cmp::Ordering::Less => Err(TextureError::TruncatedPart {
            index,
            expected,
            actual,
        }),
        std::cmp::Ordering::Greater => Err(TextureError::OversizedPart {
            index,
            expected,
            actual,
        }),
        std::cmp::Ordering::Equal => Ok(expected),
    }
}

pub(crate) fn read_prefix(
    header: &mut impl Read,
    prefix: &mut [u8],
) -> Result<usize, TextureError> {
    let mut filled = 0;
    while let Some(free) = prefix.get_mut(filled..).filter(|free| !free.is_empty()) {
        match header.read(free) {
            Ok(0) => break,
            Ok(count) => filled += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(TextureError::ReadHeader(error)),
        }
    }
    Ok(filled)
}

/// Reads up to `size` bytes, growing the buffer with the data instead of trusting `size`.
pub(crate) fn read_up_to(reader: impl Read, size: u64) -> io::Result<Vec<u8>> {
    let capacity = usize::try_from(size.min(INITIAL_CAPACITY)).unwrap_or_default();
    let mut data = Vec::with_capacity(capacity);
    reader.take(size).read_to_end(&mut data)?;
    Ok(data)
}

enum CopyError {
    Read(io::Error),
    Write(io::Error),
}

impl CopyError {
    fn into_texture_error(self, read: impl FnOnce(io::Error) -> TextureError) -> TextureError {
        match self {
            Self::Read(error) => read(error),
            Self::Write(error) => TextureError::Write(error),
        }
    }
}

fn copy<W: Write + ?Sized>(
    reader: impl Read,
    limit: u64,
    out: &mut W,
    buffer: &mut [u8],
) -> Result<u64, CopyError> {
    let mut reader = reader.take(limit);
    let mut copied = 0;
    loop {
        let count = match reader.read(buffer) {
            Ok(0) => return Ok(copied),
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(CopyError::Read(error)),
        };
        out.write_all(buffer.get(..count).unwrap_or_default())
            .map_err(CopyError::Write)?;
        copied += count as u64;
    }
}
