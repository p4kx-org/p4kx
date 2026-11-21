// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{DdsRawHeader, DxgiFormat, TextureError, TextureFormat};

const FLAG_DEPTH: u32 = 0x80_0000;
const CAPS2_CUBEMAP: u32 = 0x200;
const CAPS2_CUBEMAP_FACES: u32 = 0xFC00;
const CAPS2_VOLUME: u32 = 0x20_0000;
const CUBE_FACES: u32 = 6;
const DIMENSION_TEXTURE3D: u32 = 4;
const MISC_TEXTURECUBE: u32 = 0x4;
pub(crate) const CRYENGINE_TRAILER: &[u8; 8] = b"CExtCEnd";
const CRYENGINE_TRAILER_LEN: u64 = CRYENGINE_TRAILER.len() as u64;

/// The longest DDS header: magic, the 124-byte header and the DX10 extension.
pub const MAX_HEADER_LEN: usize = crate::dds_raw_header::MAX_HEADER_LEN;

/// What the header of a DDS texture says about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DdsHeader {
    format: TextureFormat,
    width: u32,
    height: u32,
    depth: u32,
    mip_count: u32,
    array_size: u32,
    faces: u32,
    header_len: usize,
}

pub(crate) const fn legacy_is_volume(flags: u32, caps2: u32) -> bool {
    !legacy_volume_uses_depth_flag(caps2) || flags & FLAG_DEPTH != 0
}

pub(crate) const fn legacy_volume_uses_depth_flag(caps2: u32) -> bool {
    caps2 & CAPS2_VOLUME == 0
}

pub(crate) const fn dx10_is_volume(dimension: u32) -> bool {
    dimension == DIMENSION_TEXTURE3D
}

struct Fields {
    flags: u32,
    height: u32,
    width: u32,
    depth: u32,
    mip_count: u32,
    legacy_format: TextureFormat,
    caps2: u32,
}

impl DdsHeader {
    /// Parses the legacy or DX10 header at the start of `bytes`, with or without the `DDS `
    /// magic; anything after the header is ignored.
    ///
    /// # Errors
    ///
    /// Returns [`TextureError::NotDds`] when `bytes` does not start with a DDS header,
    /// [`TextureError::TruncatedHeader`] when it ends inside the header, and
    /// [`TextureError::InvalidHeader`] when the topology contradicts itself or its byte extent overflows.
    pub fn parse(bytes: &[u8]) -> Result<Self, TextureError> {
        Self::from_raw(&DdsRawHeader::parse(bytes)?)
    }

    /// Applies the reader's topology normalization and validation to captured source fields.
    ///
    /// # Errors
    /// Returns the same invalid-topology errors as parsing the corresponding complete header.
    pub fn from_raw(raw: &DdsRawHeader) -> Result<Self, TextureError> {
        let legacy = raw.legacy();
        let pixel = legacy.pixel_format;
        let fields = Fields {
            flags: legacy.flags,
            height: legacy.height,
            width: legacy.width,
            depth: legacy.depth,
            mip_count: legacy.mip_map_count,
            legacy_format: TextureFormat::from_legacy(
                pixel.flags,
                pixel.four_cc,
                pixel.rgb_bit_count,
                [
                    pixel.r_bit_mask,
                    pixel.g_bit_mask,
                    pixel.b_bit_mask,
                    pixel.a_bit_mask,
                ],
            ),
            caps2: legacy.caps2,
        };
        let header = raw.dx10().map_or_else(
            || Self::from_legacy(&fields, raw.header_len()),
            |dx10| {
                Self::from_dx10(
                    &fields,
                    [
                        dx10.dxgi_format,
                        dx10.resource_dimension,
                        dx10.misc_flag,
                        dx10.array_size,
                    ],
                    raw.header_len(),
                )
            },
        );
        header.validated()
    }

    fn from_legacy(fields: &Fields, header_len: usize) -> Self {
        let is_volume = legacy_is_volume(fields.flags, fields.caps2);
        let faces = if fields.caps2 & CAPS2_CUBEMAP == 0 {
            1
        } else {
            (fields.caps2 & CAPS2_CUBEMAP_FACES).count_ones().max(1)
        };
        Self {
            format: fields.legacy_format,
            width: fields.width,
            height: fields.height,
            depth: if is_volume { fields.depth.max(1) } else { 1 },
            mip_count: fields.mip_count.max(1),
            array_size: 1,
            faces,
            header_len,
        }
    }

    fn from_dx10(fields: &Fields, dx10: [u32; 4], header_len: usize) -> Self {
        let [format, dimension, misc, array_size] = dx10;
        Self {
            format: TextureFormat::Dxgi(DxgiFormat(format)),
            width: fields.width,
            height: fields.height,
            depth: if dx10_is_volume(dimension) {
                fields.depth.max(1)
            } else {
                1
            },
            mip_count: fields.mip_count.max(1),
            array_size: array_size.max(1),
            faces: if misc & MISC_TEXTURECUBE == 0 {
                1
            } else {
                CUBE_FACES
            },
            header_len,
        }
    }

    fn validated(self) -> Result<Self, TextureError> {
        if self.width == 0 || self.height == 0 {
            return Err(TextureError::InvalidHeader("width or height is zero"));
        }
        if self.depth > 1 && self.surfaces() > 1 {
            return Err(TextureError::InvalidHeader(
                "a volume cannot be a cubemap or an array",
            ));
        }
        let largest = self.width.max(self.height).max(self.depth);
        if self.mip_count > largest.bit_width() {
            return Err(TextureError::InvalidHeader(
                "more mips than the size allows",
            ));
        }
        if self.format.layout().is_some() && self.mip_size(0).is_none() {
            return Err(TextureError::InvalidHeader("the texture size overflows"));
        }
        Ok(self)
    }

    /// Returns the pixel format.
    #[must_use]
    pub const fn format(&self) -> TextureFormat {
        self.format
    }

    /// Returns the width of the largest mip in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns the height of the largest mip in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns the depth of the largest mip in slices: 1 unless the texture is a volume.
    #[must_use]
    pub const fn depth(&self) -> u32 {
        self.depth
    }

    /// Returns the number of mips, at least 1.
    #[must_use]
    pub const fn mip_count(&self) -> u32 {
        self.mip_count
    }

    /// Returns the number of array elements, 1 unless a DX10 header says more.
    #[must_use]
    pub const fn array_size(&self) -> u32 {
        self.array_size
    }

    /// Returns the number of cubemap faces per array element: 6 for a full cubemap, otherwise 1.
    #[must_use]
    pub const fn faces(&self) -> u32 {
        self.faces
    }

    /// Returns whether the texture is a cubemap.
    #[must_use]
    pub const fn is_cubemap(&self) -> bool {
        self.faces > 1
    }

    /// Returns the length of the header in bytes, including the magic and any DX10 extension.
    #[must_use]
    pub const fn header_len(&self) -> usize {
        self.header_len
    }

    /// Returns the number of images per mip: faces times array elements.
    #[must_use]
    pub const fn surfaces(&self) -> u64 {
        self.faces as u64 * self.array_size as u64
    }

    /// Returns the number of slices of mip `level`: the depth halved per level, at least 1.
    #[must_use]
    pub const fn slices(&self, level: u32) -> u32 {
        match self.depth.checked_shr(level) {
            Some(slices) if slices > 0 => slices,
            _ => 1,
        }
    }

    /// Returns the number of images in mip `level`: one per cubemap face, array element and
    /// slice.
    #[must_use]
    pub const fn images(&self, level: u32) -> u64 {
        self.surfaces() * self.slices(level) as u64
    }

    /// Returns the size in bytes of mip `level` with every face, array element and slice, or
    /// `None` when the format's layout is unknown, the level does not exist or the size
    /// overflows.
    #[must_use]
    pub fn mip_size(&self, level: u32) -> Option<u64> {
        self.surface_size(level)?
            .checked_mul(u64::from(self.faces))?
            .checked_mul(u64::from(self.array_size))
    }

    /// Returns the size in bytes of mip `level` of one image, a single face or array element
    /// with every slice, or `None` when the format's layout is unknown, the level does not exist
    /// or the size overflows.
    #[must_use]
    pub fn surface_size(&self, level: u32) -> Option<u64> {
        if level >= self.mip_count {
            return None;
        }
        let layout = self.format.layout()?;
        let side = |full: u32, block: u32| u64::from((full >> level).max(1).div_ceil(block));
        side(self.width, layout.block_width)
            .checked_mul(side(self.height, layout.block_height))?
            .checked_mul(u64::from(layout.bytes_per_block))?
            .checked_mul(u64::from(self.slices(level)))
    }

    /// Returns the size in bytes of mips `levels` of one image, or `None` when one of them has
    /// no size.
    pub(crate) fn surface_chain_size(&self, levels: std::ops::Range<u32>) -> Option<u64> {
        levels
            .map(|level| self.surface_size(level))
            .try_fold(0_u64, |sum, size| sum.checked_add(size?))
    }

    /// Returns the number of sibling parts a header Entry of `header_size` bytes is split into:
    /// the mips that do not follow the header in the Entry itself, one per part. Exactly 8 bytes
    /// after the last mip are taken for the CryEngine trailer `CExtCEnd` and are not a mip; only
    /// their size is checked here, as the header Entry's bytes are not read. [`merge`] and the
    /// header-tail path of [`read_mip`] verify them; the part path of [`read_mip`] does not.
    ///
    /// [`merge`]: crate::merge
    /// [`read_mip`]: crate::read_mip
    ///
    /// # Errors
    ///
    /// Returns [`TextureError::UnsupportedFormat`] when the format's layout is unknown,
    /// [`TextureError::TruncatedHeader`] when `header_size` is shorter than the header, and
    /// [`TextureError::TailMismatch`] when the bytes after the header are not the smallest mips.
    pub fn part_count(&self, header_size: u64) -> Result<u32, TextureError> {
        if self.format.layout().is_none() {
            return Err(TextureError::UnsupportedFormat(self.format));
        }
        let tail = header_size.checked_sub(self.header_len as u64).ok_or(
            TextureError::TruncatedHeader {
                len: header_size,
                needed: self.header_len as u64,
            },
        )?;
        self.parts_for(tail)
            .or_else(|| self.parts_for(tail.checked_sub(CRYENGINE_TRAILER_LEN)?))
            .ok_or(TextureError::TailMismatch { tail })
    }

    /// Returns the length of the data of a header Entry of `header_size` bytes without the
    /// 8-byte CryEngine trailer that follows the mips of some textures, or `header_size` when the
    /// Entry has none. Judged by size only; the trailer's bytes are not read.
    #[must_use]
    pub fn data_len(&self, header_size: u64) -> u64 {
        let trailer_free = header_size
            .checked_sub(self.header_len as u64)
            .filter(|&tail| self.parts_for(tail).is_none())
            .and_then(|tail| self.parts_for(tail.checked_sub(CRYENGINE_TRAILER_LEN)?))
            .map(|_| header_size - CRYENGINE_TRAILER_LEN);
        trailer_free.unwrap_or(header_size)
    }

    fn parts_for(&self, tail: u64) -> Option<u32> {
        let mut sum = 0_u64;
        for level in (0..self.mip_count).rev() {
            if sum == tail {
                return Some(level + 1);
            }
            sum = sum.checked_add(self.mip_size(level)?)?;
        }
        (sum == tail).then_some(0)
    }
}
