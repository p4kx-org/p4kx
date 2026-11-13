// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_texture::{DdsDx10Fields, DdsLegacyFields, DdsPixelFormatFields};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The complete authored legacy DDS words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct DdsLegacyInfo {
    /// Header structure size word.
    #[schemars(range(max = u32::MAX))]
    pub size: u32,
    /// Unmodified validity flags.
    #[schemars(range(max = u32::MAX))]
    pub flags: u32,
    /// Authored pixel height.
    #[schemars(range(max = u32::MAX))]
    pub height: u32,
    /// Authored pixel width.
    #[schemars(range(max = u32::MAX))]
    pub width: u32,
    /// Authored pitch or linear-size word.
    #[schemars(range(max = u32::MAX))]
    pub pitch_or_linear_size: u32,
    /// Authored depth before defaults.
    #[schemars(range(max = u32::MAX))]
    pub depth: u32,
    /// Authored mip count before defaults.
    #[schemars(range(max = u32::MAX))]
    pub mip_map_count: u32,
    /// Uninterpreted reserved words in source order.
    #[schemars(inner(range(max = u32::MAX)))]
    pub reserved1: [u32; 11],
    /// Complete authored pixel-format words.
    pub pixel_format: DdsPixelFormatInfo,
    /// Unmodified surface capability word.
    #[schemars(range(max = u32::MAX))]
    pub caps: u32,
    /// Unmodified additional capability word.
    #[schemars(range(max = u32::MAX))]
    pub caps2: u32,
    /// Uninterpreted capability word.
    #[schemars(range(max = u32::MAX))]
    pub caps3: u32,
    /// Uninterpreted capability word.
    #[schemars(range(max = u32::MAX))]
    pub caps4: u32,
    /// Uninterpreted final reserved word.
    #[schemars(range(max = u32::MAX))]
    pub reserved2: u32,
}

/// The eight authored legacy pixel-format words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct DdsPixelFormatInfo {
    /// Authored pixel-format structure size.
    #[schemars(range(max = u32::MAX))]
    pub size: u32,
    /// Unmodified pixel-format flags.
    #[schemars(range(max = u32::MAX))]
    pub flags: u32,
    /// Exact original bytes without a text or numeric-format guess.
    pub four_cc: [u8; 4],
    /// Authored bit-count word.
    #[schemars(range(max = u32::MAX))]
    pub rgb_bit_count: u32,
    /// Authored red mask.
    #[schemars(range(max = u32::MAX))]
    pub r_bit_mask: u32,
    /// Authored green mask.
    #[schemars(range(max = u32::MAX))]
    pub g_bit_mask: u32,
    /// Authored blue mask.
    #[schemars(range(max = u32::MAX))]
    pub b_bit_mask: u32,
    /// Authored alpha mask.
    #[schemars(range(max = u32::MAX))]
    pub a_bit_mask: u32,
}

/// The five authored DX10 extension words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct DdsDx10Info {
    /// Unmodified DXGI format number.
    #[schemars(range(max = u32::MAX))]
    pub dxgi_format: u32,
    /// Unmodified resource-dimension number.
    #[schemars(range(max = u32::MAX))]
    pub resource_dimension: u32,
    /// Unmodified miscellaneous flag word.
    #[schemars(range(max = u32::MAX))]
    pub misc_flag: u32,
    /// Authored array size before defaults.
    #[schemars(range(max = u32::MAX))]
    pub array_size: u32,
    /// Unmodified additional miscellaneous flags.
    #[schemars(range(max = u32::MAX))]
    pub misc_flags2: u32,
}

impl DdsLegacyInfo {
    pub(crate) const fn of(raw: &DdsLegacyFields) -> Self {
        Self {
            size: raw.size,
            flags: raw.flags,
            height: raw.height,
            width: raw.width,
            pitch_or_linear_size: raw.pitch_or_linear_size,
            depth: raw.depth,
            mip_map_count: raw.mip_map_count,
            reserved1: raw.reserved1,
            pixel_format: DdsPixelFormatInfo::of(&raw.pixel_format),
            caps: raw.caps,
            caps2: raw.caps2,
            caps3: raw.caps3,
            caps4: raw.caps4,
            reserved2: raw.reserved2,
        }
    }
}

impl DdsPixelFormatInfo {
    pub(crate) const fn of(raw: &DdsPixelFormatFields) -> Self {
        Self {
            size: raw.size,
            flags: raw.flags,
            four_cc: raw.four_cc,
            rgb_bit_count: raw.rgb_bit_count,
            r_bit_mask: raw.r_bit_mask,
            g_bit_mask: raw.g_bit_mask,
            b_bit_mask: raw.b_bit_mask,
            a_bit_mask: raw.a_bit_mask,
        }
    }
}

impl DdsDx10Info {
    pub(crate) const fn of(raw: &DdsDx10Fields) -> Self {
        Self {
            dxgi_format: raw.dxgi_format,
            resource_dimension: raw.resource_dimension,
            misc_flag: raw.misc_flag,
            array_size: raw.array_size,
            misc_flags2: raw.misc_flags2,
        }
    }
}
