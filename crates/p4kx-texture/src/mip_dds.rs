// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use crate::header::{dx10_is_volume, legacy_is_volume};
use crate::{
    DdsHeader, DdsLegacyFields, DdsRawHeader, LocatedMip, MAX_HEADER_LEN, Mip, TextureError,
};

const DDS_MAGIC: &[u8; 4] = b"DDS ";
const DDSD_TEXTURE: u32 = 0x1007;
const DDSD_PITCH: u32 = 0x8;
const DDSD_LINEAR_SIZE: u32 = 0x0008_0000;
const DDSD_DEPTH: u32 = 0x0080_0000;
const DDSCAPS_TEXTURE: u32 = 0x1000;
const DDSCAPS_COMPLEX: u32 = 0x8;
const DDSCAPS2_CUBE_FACES: u32 = 0xfe00;
const DDSCAPS2_VOLUME: u32 = 0x0020_0000;

impl LocatedMip {
    /// Writes one original mip as standard DDS with its complete surface set and unchanged pixels.
    ///
    /// # Errors
    /// Returns an invalid topology, unrepresentable pitch or size, or write error.
    pub fn write_dds(&self, out: &mut (impl Write + ?Sized)) -> Result<(), TextureError> {
        let header = DdsHeader::from_raw(&self.raw_header)?;
        if &header != self.mip.header() {
            return Err(TextureError::InvalidHeader(
                "the original and selected mip headers differ",
            ));
        }
        let bytes = mip_header(&self.raw_header, &self.mip)?;
        out.write_all(&bytes).map_err(TextureError::Write)?;
        out.write_all(self.mip.data()).map_err(TextureError::Write)
    }
}

fn mip_header(raw: &DdsRawHeader, mip: &Mip) -> Result<Vec<u8>, TextureError> {
    let header = normalized_fields(raw, mip)?;
    let pixel = header.pixel_format;
    let mut bytes = Vec::with_capacity(MAX_HEADER_LEN);
    bytes.extend_from_slice(DDS_MAGIC);
    append_words(
        &mut bytes,
        [
            header.size,
            header.flags,
            header.height,
            header.width,
            header.pitch_or_linear_size,
            header.depth,
            header.mip_map_count,
        ]
        .into_iter()
        .chain(header.reserved1)
        .chain([
            pixel.size,
            pixel.flags,
            u32::from_le_bytes(pixel.four_cc),
            pixel.rgb_bit_count,
            pixel.r_bit_mask,
            pixel.g_bit_mask,
            pixel.b_bit_mask,
            pixel.a_bit_mask,
            header.caps,
            header.caps2,
            header.caps3,
            header.caps4,
            header.reserved2,
        ]),
    );
    if let Some(dx10) = raw.dx10() {
        append_words(
            &mut bytes,
            [
                dx10.dxgi_format,
                dx10.resource_dimension,
                dx10.misc_flag,
                mip.header().array_size(),
                dx10.misc_flags2,
            ],
        );
    }
    Ok(bytes)
}

fn normalized_fields(raw: &DdsRawHeader, mip: &Mip) -> Result<DdsLegacyFields, TextureError> {
    let (layout, pitch) = layout_and_pitch(mip)?;
    let mut header = *raw.legacy();
    let dx10 = raw.dx10();
    let volume = dx10.map_or_else(
        || {
            legacy_is_volume(header.flags, header.caps2)
                && (header.depth > 1 || header.caps2 & DDSCAPS2_VOLUME != 0)
        },
        |dx10| dx10_is_volume(dx10.resource_dimension),
    );
    header.flags = DDSD_TEXTURE | layout;
    header.width = mip.width();
    header.height = mip.height();
    header.pitch_or_linear_size = pitch;
    header.depth = if volume {
        mip.header().slices(mip.level())
    } else {
        0
    };
    header.mip_map_count = 1;
    header.caps = DDSCAPS_TEXTURE;
    header.caps2 = if mip.header().is_cubemap() {
        if dx10.is_some() {
            DDSCAPS2_CUBE_FACES
        } else {
            header.caps2 & DDSCAPS2_CUBE_FACES
        }
    } else {
        0
    };
    if volume {
        header.flags |= DDSD_DEPTH;
        header.caps2 |= DDSCAPS2_VOLUME;
    }
    if volume || mip.header().is_cubemap() || mip.header().array_size() > 1 {
        header.caps |= DDSCAPS_COMPLEX;
    }
    Ok(header)
}

fn layout_and_pitch(mip: &Mip) -> Result<(u32, u32), TextureError> {
    let format = mip.header().format();
    let layout = format
        .layout()
        .ok_or(TextureError::UnsupportedFormat(format))?;
    let row =
        u64::from(mip.width().div_ceil(layout.block_width)) * u64::from(layout.bytes_per_block);
    let (flag, pitch) = if layout.block_width == 1 && layout.block_height == 1 {
        (DDSD_PITCH, Some(row))
    } else {
        (
            DDSD_LINEAR_SIZE,
            row.checked_mul(u64::from(mip.height().div_ceil(layout.block_height))),
        )
    };
    let pitch =
        pitch
            .and_then(|pitch| u32::try_from(pitch).ok())
            .ok_or(TextureError::InvalidHeader(
                "the selected DDS mip pitch exceeds 32 bits",
            ))?;
    Ok((flag, pitch))
}

fn append_words(bytes: &mut Vec<u8>, words: impl IntoIterator<Item = u32>) {
    bytes.extend(words.into_iter().flat_map(u32::to_le_bytes));
}
