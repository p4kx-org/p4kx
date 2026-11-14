// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use crate::DxgiFormat;

const PF_ALPHA_PIXELS: u32 = 0x1;
const PF_ALPHA: u32 = 0x2;
const PF_FOURCC: u32 = 0x4;
const PF_RGB: u32 = 0x40;
const PF_LUMINANCE: u32 = 0x2_0000;

/// How the pixels of one mip are stored: blocks of `block_width` × `block_height` pixels, each
/// `bytes_per_block` bytes long.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockLayout {
    /// Width of one block in pixels: 4 for block-compressed formats, otherwise 1.
    pub block_width: u32,
    /// Height of one block in pixels.
    pub block_height: u32,
    /// Bytes per block.
    pub bytes_per_block: u32,
}

/// The pixel format of a DDS texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureFormat {
    /// A DXGI format, from a DX10 header or the DXGI equivalent of a legacy one.
    Dxgi(DxgiFormat),
    /// 24-bit BGR from a legacy header, which has no DXGI equivalent.
    Bgr8,
    /// 8-bit luminance and 8-bit alpha from a legacy header, which has no DXGI equivalent.
    L8A8,
    /// A legacy `FourCC` this version does not know.
    UnknownFourCc([u8; 4]),
    /// Legacy bit masks this version does not know.
    UnknownMasks {
        /// Bits per pixel.
        bit_count: u32,
        /// The red, green, blue and alpha masks.
        masks: [u32; 4],
    },
}

impl TextureFormat {
    /// Returns how pixels are stored, or `None` when the format is unknown.
    #[must_use]
    pub fn layout(self) -> Option<BlockLayout> {
        match self {
            Self::Dxgi(format) => format.layout(),
            Self::Bgr8 => Some(BlockLayout {
                block_width: 1,
                block_height: 1,
                bytes_per_block: 3,
            }),
            Self::L8A8 => Some(BlockLayout {
                block_width: 1,
                block_height: 1,
                bytes_per_block: 2,
            }),
            Self::UnknownFourCc(_) | Self::UnknownMasks { .. } => None,
        }
    }

    /// Returns the format a legacy pixel format describes.
    pub(crate) const fn from_legacy(
        flags: u32,
        four_cc: [u8; 4],
        bit_count: u32,
        masks: [u32; 4],
    ) -> Self {
        if legacy_uses_four_cc(flags) {
            return from_four_cc(four_cc);
        }
        let [red, green, blue, alpha_mask] = masks;
        let alpha = if legacy_uses_alpha_mask(flags) {
            alpha_mask
        } else {
            0
        };
        let dxgi = match (bit_count, [red, green, blue, alpha]) {
            (32, [0xFF_0000, 0xFF00, 0xFF, 0xFF00_0000]) => 87,
            (32, [0xFF_0000, 0xFF00, 0xFF, 0]) => 88,
            (32, [0xFF, 0xFF00, 0xFF_0000, 0xFF00_0000]) => 28,
            (32, [0xFFFF, 0xFFFF_0000, 0, 0]) => 35,
            (32, [0x3FF, 0xF_FC00, 0x3FF0_0000, 0xC000_0000]) => 24,
            (24, [0xFF_0000, 0xFF00, 0xFF, 0]) => return Self::Bgr8,
            (16, [0xF800, 0x7E0, 0x1F, 0]) => 85,
            (16, [0x7C00, 0x3E0, 0x1F, 0x8000]) => 86,
            (16, [0xF00, 0xF0, 0xF, 0xF000]) => 115,
            (16, [0xFF, 0, 0, 0xFF00]) if flags & PF_LUMINANCE != 0 => return Self::L8A8,
            (16, [0xFFFF, 0, 0, 0]) => 56,
            (8, [0xFF, 0, 0, 0]) => 61,
            (8, [0, 0, 0, 0xFF]) if flags & PF_RGB == 0 => 65,
            _ => return Self::UnknownMasks { bit_count, masks },
        };
        Self::Dxgi(DxgiFormat(dxgi))
    }
}

pub(crate) const fn legacy_uses_four_cc(flags: u32) -> bool {
    flags & PF_FOURCC != 0
}

pub(crate) const fn legacy_uses_alpha_mask(flags: u32) -> bool {
    flags & (PF_ALPHA_PIXELS | PF_ALPHA) != 0
}

const fn from_four_cc(four_cc: [u8; 4]) -> TextureFormat {
    let dxgi = match &four_cc {
        b"DXT1" => DxgiFormat::BC1_UNORM,
        b"DXT2" | b"DXT3" => DxgiFormat::BC2_UNORM,
        b"DXT4" | b"DXT5" => DxgiFormat::BC3_UNORM,
        b"ATI1" | b"BC4U" => DxgiFormat::BC4_UNORM,
        b"BC4S" => DxgiFormat::BC4_SNORM,
        b"ATI2" | b"BC5U" => DxgiFormat::BC5_UNORM,
        b"BC5S" => DxgiFormat::BC5_SNORM,
        _ => match u32::from_le_bytes(four_cc) {
            36 => DxgiFormat(11),
            110 => DxgiFormat(13),
            111 => DxgiFormat(54),
            112 => DxgiFormat(34),
            113 => DxgiFormat(10),
            114 => DxgiFormat(41),
            115 => DxgiFormat(16),
            116 => DxgiFormat(2),
            _ => return TextureFormat::UnknownFourCc(four_cc),
        },
    };
    TextureFormat::Dxgi(dxgi)
}

impl fmt::Display for TextureFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dxgi(format) => match format.name() {
                Some(name) => f.write_str(name),
                None => write!(f, "DXGI format {}", format.0),
            },
            Self::Bgr8 => f.write_str("B8G8R8_UNORM"),
            Self::L8A8 => f.write_str("L8A8_UNORM"),
            Self::UnknownFourCc(four_cc) => {
                write!(f, "FourCC {}", four_cc.escape_ascii())
            }
            Self::UnknownMasks {
                bit_count,
                masks: [red, green, blue, alpha],
            } => write!(
                f,
                "{bit_count}-bit masks {red:#x} {green:#x} {blue:#x} {alpha:#x}"
            ),
        }
    }
}
