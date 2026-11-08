// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// Authored legacy DDS words without defaults or bit reinterpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct DdsLegacyFields {
    /// Header structure size word.
    pub size: u32,
    /// Unmodified validity flags.
    pub flags: u32,
    /// Authored pixel height.
    pub height: u32,
    /// Authored pixel width.
    pub width: u32,
    /// Authored pitch or linear-size word.
    pub pitch_or_linear_size: u32,
    /// Authored depth, including zero when unused.
    pub depth: u32,
    /// Authored mip count, including zero.
    pub mip_map_count: u32,
    /// Eleven uninterpreted reserved words in source order.
    pub reserved1: [u32; 11],
    /// Authored pixel-format words.
    pub pixel_format: DdsPixelFormatFields,
    /// Unmodified surface capability word.
    pub caps: u32,
    /// Unmodified additional capability word.
    pub caps2: u32,
    /// Uninterpreted capability word.
    pub caps3: u32,
    /// Uninterpreted capability word.
    pub caps4: u32,
    /// Uninterpreted final reserved word.
    pub reserved2: u32,
}

/// The eight authored legacy pixel-format words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct DdsPixelFormatFields {
    /// Authored pixel-format structure size.
    pub size: u32,
    /// Unmodified pixel-format flags.
    pub flags: u32,
    /// Exact four original bytes, including nontext and numeric `FourCC` values.
    pub four_cc: [u8; 4],
    /// Authored bit-count word.
    pub rgb_bit_count: u32,
    /// Authored red bit mask.
    pub r_bit_mask: u32,
    /// Authored green bit mask.
    pub g_bit_mask: u32,
    /// Authored blue bit mask.
    pub b_bit_mask: u32,
    /// Authored alpha bit mask.
    pub a_bit_mask: u32,
}

/// The five authored DX10 extension words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct DdsDx10Fields {
    /// Unmodified DXGI format number.
    pub dxgi_format: u32,
    /// Unmodified resource-dimension number.
    pub resource_dimension: u32,
    /// Unmodified miscellaneous flag word.
    pub misc_flag: u32,
    /// Authored array size, including zero.
    pub array_size: u32,
    /// Unmodified additional miscellaneous flags.
    pub misc_flags2: u32,
}
