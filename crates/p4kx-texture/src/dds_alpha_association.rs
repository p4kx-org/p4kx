// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::DdsRawHeader;
use crate::format::legacy_uses_four_cc;

const DX10_ALPHA_MODE_MASK: u32 = 0x7;

/// Association of the stored colour channels with alpha declared by the DDS header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DdsAlphaAssociation {
    /// The header does not declare an association.
    Unknown,
    /// RGB is independent of the alpha channel.
    Straight,
    /// RGB has been multiplied by alpha.
    Premultiplied,
    /// The consumer should treat the texture as opaque.
    Opaque,
    /// The fourth channel is authored as data.
    Custom,
    /// The alpha-mode bits contain an undefined value.
    Reserved,
}

impl DdsRawHeader {
    /// Returns the association declared by the original `FourCC` or DX10 alpha-mode bits.
    #[must_use]
    pub const fn alpha_association(&self) -> DdsAlphaAssociation {
        use DdsAlphaAssociation::{Custom, Opaque, Premultiplied, Reserved, Straight, Unknown};

        if let Some(dx10) = self.dx10() {
            return match dx10.misc_flags2 & DX10_ALPHA_MODE_MASK {
                0 => Unknown,
                1 => Straight,
                2 => Premultiplied,
                3 => Opaque,
                4 => Custom,
                _ => Reserved,
            };
        }
        let pixel = &self.legacy().pixel_format;
        if !legacy_uses_four_cc(pixel.flags) {
            return Unknown;
        }
        match &pixel.four_cc {
            b"DXT2" | b"DXT4" => Premultiplied,
            b"DXT3" | b"DXT5" => Straight,
            _ => Unknown,
        }
    }
}
