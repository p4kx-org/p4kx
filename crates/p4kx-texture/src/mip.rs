// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};

use rayon::prelude::*;

use crate::header::MAX_HEADER_LEN;
use crate::merge::{check_trailer, checked_part_len, read_prefix, read_up_to};
use crate::mip_source::{LocatedMip, MipPayloadLocation};
use crate::{
    DdsHeader, DdsRawHeader, HdrImage, RgbaImage, SiblingReader, TextureError, decode, decode_hdr,
};

/// The data of one mip of a texture: one image, or one per cubemap face or array element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mip {
    header: DdsHeader,
    level: u32,
    data: Vec<u8>,
}

impl Mip {
    /// Returns the header of the texture.
    #[must_use]
    pub const fn header(&self) -> &DdsHeader {
        &self.header
    }

    /// Returns the mip level, 0 being the largest.
    #[must_use]
    pub const fn level(&self) -> u32 {
        self.level
    }

    /// Returns the width of the mip in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        side(self.header.width(), self.level)
    }

    /// Returns the height of the mip in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        side(self.header.height(), self.level)
    }

    /// Returns the number of images: 6 for a cubemap, the array size for an array, the slices of
    /// this mip for a volume, otherwise 1; the product when a texture is several of these.
    #[must_use]
    pub const fn surfaces(&self) -> u64 {
        self.header.images(self.level)
    }

    /// Returns the mip's data in the texture's format, image after image.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Returns the data of image `index`: in DDS order +X, −X, +Y, −Y, +Z, −Z for a cubemap,
    /// element by element for an array, slice by slice for a volume; `None` when there is no
    /// such image.
    #[must_use]
    pub fn surface(&self, index: u64) -> Option<&[u8]> {
        if index >= self.surfaces() {
            return None;
        }
        let len = self.data.len() / usize::try_from(self.surfaces()).ok()?;
        let start = len.checked_mul(usize::try_from(index).ok()?)?;
        self.data.get(start..start.checked_add(len)?)
    }

    /// Decodes the mip to 8-bit RGBA.
    ///
    /// # Errors
    ///
    /// Returns [`TextureError::UnsupportedImage`] when the mip holds several images, and the
    /// errors of [`decode`].
    pub fn decode(&self) -> Result<RgbaImage, TextureError> {
        if self.surfaces() > 1 {
            return Err(TextureError::UnsupportedImage {
                images: self.surfaces(),
            });
        }
        decode(
            self.header.format(),
            self.width(),
            self.height(),
            &self.data,
        )
    }

    /// Decodes the mip of a high-range texture to floating-point RGB without clipping.
    ///
    /// # Errors
    ///
    /// Returns [`TextureError::UnsupportedImage`] when the mip holds several images, and the
    /// errors of [`decode_hdr`].
    pub fn decode_hdr(&self) -> Result<HdrImage, TextureError> {
        if self.surfaces() > 1 {
            return Err(TextureError::UnsupportedImage {
                images: self.surfaces(),
            });
        }
        decode_hdr(
            self.header.format(),
            self.width(),
            self.height(),
            &self.data,
        )
    }

    /// Decodes every image of the mip of a high-range texture to floating-point RGB, in
    /// parallel.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`decode_hdr`].
    pub fn decode_surfaces_hdr(&self) -> Result<Vec<HdrImage>, TextureError> {
        (0..self.surfaces())
            .into_par_iter()
            .map(|index| {
                decode_hdr(
                    self.header.format(),
                    self.width(),
                    self.height(),
                    self.surface(index).unwrap_or_default(),
                )
            })
            .collect()
    }

    /// Decodes every image of the mip to 8-bit RGBA, in parallel.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`decode`].
    pub fn decode_surfaces(&self) -> Result<Vec<RgbaImage>, TextureError> {
        (0..self.surfaces())
            .into_par_iter()
            .map(|index| {
                decode(
                    self.header.format(),
                    self.width(),
                    self.height(),
                    self.surface(index).unwrap_or_default(),
                )
            })
            .collect()
    }
}

const fn side(full: u32, level: u32) -> u32 {
    match full.checked_shr(level) {
        Some(side) if side > 0 => side,
        _ => 1,
    }
}

/// Reads mip `level`, 0 being the largest, of the header Entry `header` of `header_size` bytes.
///
/// The mip comes from its sibling part when it is split off, otherwise from the header's tail,
/// with every cubemap face, array element and volume slice. The 8-byte CryEngine trailer
/// `CExtCEnd` some header Entries carry after the last mip is verified when the mip comes from
/// the header's tail, by reading on to the end of the data; when it comes from a part, only the
/// trailer's size is checked, as verifying it would need an extra read of the header Entry.
///
/// # Errors
///
/// Returns [`TextureError::NoSuchMip`] when the level does not exist, the errors of
/// [`DdsHeader::part_count`], [`TextureError::UnknownTrailer`] when the header's tail is read and
/// the trailer is not `CExtCEnd`, and a part or header error when the mip's data is missing,
/// truncated, oversized or unreadable.
pub fn read_mip<R, S>(
    header: R,
    header_size: u64,
    siblings: &S,
    level: u32,
) -> Result<Mip, TextureError>
where
    R: Read,
    S: SiblingReader + ?Sized,
{
    read_mip_with_location(header, header_size, siblings, level).map(|read| read.mip)
}

/// Reads one mip and returns the original source spans of its retained bytes.
///
/// # Errors
///
/// Returns the errors of a mip read.
pub fn read_mip_with_location<R, S>(
    header: R,
    header_size: u64,
    siblings: &S,
    level: u32,
) -> Result<LocatedMip, TextureError>
where
    R: Read,
    S: SiblingReader + ?Sized,
{
    read_located(header, header_size, siblings, level, ReadExtent::Selected)
}

impl LocatedMip {
    /// Reads a selected mip and drains its original header and used sibling to checked EOF.
    ///
    /// # Errors
    /// Returns the source reader, format or selected mip errors.
    pub fn read_complete(
        header: impl Read,
        header_size: u64,
        siblings: &(impl SiblingReader + ?Sized),
        level: u32,
    ) -> Result<Self, TextureError> {
        read_located(
            header,
            header_size,
            siblings,
            level,
            ReadExtent::CompleteEntries,
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReadExtent {
    Selected,
    CompleteEntries,
}

fn read_located(
    mut header: impl Read,
    header_size: u64,
    siblings: &(impl SiblingReader + ?Sized),
    level: u32,
    extent: ReadExtent,
) -> Result<LocatedMip, TextureError> {
    let mut prefix = [0; MAX_HEADER_LEN];
    let limit =
        usize::try_from(header_size).map_or(MAX_HEADER_LEN, |size| size.min(MAX_HEADER_LEN));
    let prefix_len = read_prefix(&mut header, prefix.get_mut(..limit).unwrap_or_default())?;
    let start = prefix.get(..prefix_len).unwrap_or_default();
    let raw_header = DdsRawHeader::parse(start)?;
    let dds = DdsHeader::from_raw(&raw_header)?;
    if level >= dds.mip_count() {
        return Err(TextureError::NoSuchMip {
            level,
            mip_count: dds.mip_count(),
        });
    }
    let entry_size = header_size;
    let header_size = dds.data_len(entry_size);
    let trailer_len = entry_size.saturating_sub(header_size);
    let parts = dds.part_count(header_size)?;
    let (data, location) = if level < parts {
        let size = dds
            .mip_size(level)
            .ok_or_else(|| TextureError::UnsupportedFormat(dds.format()))?;
        let index = parts - level;
        let data = read_from_part(&dds, parts, index, size, siblings, extent)?;
        if extent == ReadExtent::CompleteEntries && trailer_len > 0 {
            let tail = start
                .get(dds.header_len()..)
                .unwrap_or_default()
                .chain(&mut header);
            verify_split_trailer(tail, dds.header_len() as u64, header_size, trailer_len)?;
        }
        let location = MipPayloadLocation::part(index, data.len() as u64, &dds, level);
        (data, location)
    } else {
        let tail = start.get(dds.header_len()..).unwrap_or_default();
        read_from_tail(
            &dds,
            parts,
            level,
            tail.chain(&mut header),
            (header_size, trailer_len),
        )?
    };
    if extent == ReadExtent::CompleteEntries {
        io::copy(&mut header, &mut io::sink()).map_err(TextureError::ReadHeader)?;
    }
    Ok(LocatedMip {
        mip: Mip {
            header: dds,
            level,
            data,
        },
        raw_header,
        payload: location,
    })
}

fn verify_split_trailer(
    mut tail: impl Read,
    prefix_size: u64,
    header_size: u64,
    trailer_len: u64,
) -> Result<(), TextureError> {
    let distance = header_size - prefix_size;
    let skipped = io::copy(&mut tail.by_ref().take(distance), &mut io::sink())
        .map_err(TextureError::ReadHeader)?;
    if skipped < distance {
        return Err(TextureError::TruncatedHeader {
            len: prefix_size + skipped,
            needed: header_size,
        });
    }
    check_trailer(tail, trailer_len, header_size)
}

fn read_from_part<S>(
    dds: &DdsHeader,
    parts: u32,
    index: u32,
    size: u64,
    siblings: &S,
    extent: ReadExtent,
) -> Result<Vec<u8>, TextureError>
where
    S: SiblingReader + ?Sized,
{
    checked_part_len(dds, parts, index, siblings)?;
    let mut reader = siblings
        .open_part(index)
        .map_err(|source| TextureError::ReadPart { index, source })?;
    let data =
        read_up_to(&mut reader, size).map_err(|source| TextureError::ReadPart { index, source })?;
    if (data.len() as u64) < size {
        return Err(TextureError::TruncatedPart {
            index,
            expected: size,
            actual: data.len() as u64,
        });
    }
    if extent == ReadExtent::CompleteEntries {
        io::copy(&mut reader, &mut io::sink())
            .map_err(|source| TextureError::ReadPart { index, source })?;
    }
    Ok(data)
}

/// Reads mip `level` of every image from the header's tail, where each image keeps its mips
/// `parts..` together. `data_trailer` is the length of the data and of the CryEngine trailer
/// behind it; the trailer is verified by skipping the last image's smaller mips and reading its
/// bytes from the same reader.
fn read_from_tail(
    dds: &DdsHeader,
    parts: u32,
    level: u32,
    mut tail: impl Read,
    data_trailer: (u64, u64),
) -> Result<(Vec<u8>, MipPayloadLocation), TextureError> {
    let (header_size, trailer_len) = data_trailer;
    let unsupported = || TextureError::UnsupportedFormat(dds.format());
    let size = dds.surface_size(level).ok_or_else(unsupported)?;
    let before = dds
        .surface_chain_size(parts..level)
        .ok_or_else(unsupported)?;
    let after = dds
        .surface_chain_size(level + 1..dds.mip_count())
        .ok_or_else(unsupported)?;
    let between = after.checked_add(before).ok_or_else(unsupported)?;
    let mut data = Vec::new();
    let mut position = dds.header_len() as u64;
    for surface in 0..dds.surfaces() {
        let skip = if surface == 0 { before } else { between };
        let skipped = io::copy(&mut tail.by_ref().take(skip), &mut io::sink())
            .map_err(TextureError::ReadHeader)?;
        let read = tail
            .by_ref()
            .take(size)
            .read_to_end(&mut data)
            .map_err(TextureError::ReadHeader)? as u64;
        position = position.saturating_add(skipped).saturating_add(read);
        if skipped < skip || read < size {
            return Err(TextureError::TruncatedHeader {
                len: position,
                needed: header_size,
            });
        }
    }
    if trailer_len > 0 {
        let skipped = io::copy(&mut tail.by_ref().take(after), &mut io::sink())
            .map_err(TextureError::ReadHeader)?;
        if skipped < after {
            return Err(TextureError::TruncatedHeader {
                len: position.saturating_add(skipped),
                needed: header_size,
            });
        }
        check_trailer(tail, trailer_len, header_size)?;
    }
    let offset = (dds.header_len() as u64)
        .checked_add(before)
        .ok_or_else(unsupported)?;
    let location = MipPayloadLocation::tail(dds, level, offset, size, between)?;
    Ok((data, location))
}
