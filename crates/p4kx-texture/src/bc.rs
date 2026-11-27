// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// The 16 texels of one decoded 4×4 block, RGBA, row by row.
pub(crate) type Texels = [[u8; 4]; 16];

/// Decodes a BC1 block, where `c0 <= c1` selects three colours and transparent black.
pub(crate) fn bc1(block: [u8; 8]) -> Texels {
    colour_block(block, true)
}

/// Decodes a BC2 block: 4-bit explicit alpha followed by a four-colour block.
pub(crate) fn bc2(block: [u8; 16]) -> Texels {
    let (alpha, colour) = split(block);
    let mut texels = colour_block(colour, false);
    for (texel, value) in texels.iter_mut().zip(bc2_alpha(alpha)) {
        texel[3] = value;
    }
    texels
}

/// Decodes a BC3 block: interpolated alpha followed by a four-colour block.
pub(crate) fn bc3(block: [u8; 16]) -> Texels {
    let (alpha, colour) = split(block);
    let mut texels = colour_block(colour, false);
    for (texel, value) in texels.iter_mut().zip(bc3_alpha(alpha)) {
        texel[3] = value;
    }
    texels
}

pub(crate) fn bc1_alpha(block: [u8; 8]) -> [u8; 16] {
    let [c0_low, c0_high, c1_low, c1_high, rows @ ..] = block;
    let c0 = u16::from_le_bytes([c0_low, c0_high]);
    let c1 = u16::from_le_bytes([c1_low, c1_high]);
    if !bc1_has_transparent_selector(c0, c1) {
        return [u8::MAX; 16];
    }
    let indices = u32::from_le_bytes(rows);
    std::array::from_fn(|index| {
        if (indices >> (index * 2)) & 0x3 == 0x3 {
            0
        } else {
            u8::MAX
        }
    })
}

pub(crate) fn bc2_block_alpha(block: [u8; 16]) -> [u8; 16] {
    bc2_alpha(split(block).0)
}

pub(crate) fn bc3_block_alpha(block: [u8; 16]) -> [u8; 16] {
    bc3_alpha(split(block).0)
}

fn bc2_alpha(alpha: [u8; 8]) -> [u8; 16] {
    let bits = u64::from_le_bytes(alpha);
    std::array::from_fn(|index| {
        let nibble = u8::try_from((bits >> (index * 4)) & 0xF).unwrap_or_default();
        nibble * 17
    })
}

fn bc3_alpha(alpha: [u8; 8]) -> [u8; 16] {
    let palette = alpha_palette(alpha[0], alpha[1]);
    let bits = u64::from_le_bytes(alpha) >> 16;
    std::array::from_fn(|index| {
        let code = usize::try_from((bits >> (index * 3)) & 0x7).unwrap_or_default();
        palette.get(code).copied().unwrap_or_default()
    })
}

const fn bc1_has_transparent_selector(c0: u16, c1: u16) -> bool {
    c0 <= c1
}

const fn split(block: [u8; 16]) -> ([u8; 8], [u8; 8]) {
    let [a0, a1, a2, a3, a4, a5, a6, a7, colour @ ..] = block;
    ([a0, a1, a2, a3, a4, a5, a6, a7], colour)
}

fn colour_block(block: [u8; 8], bc1_mode: bool) -> Texels {
    let [c0_low, c0_high, c1_low, c1_high, rows @ ..] = block;
    let c0 = u16::from_le_bytes([c0_low, c0_high]);
    let c1 = u16::from_le_bytes([c1_low, c1_high]);
    let palette = colour_palette(c0, c1, bc1_mode);
    let indices = u32::from_le_bytes(rows);
    let mut texels = [[0; 4]; 16];
    for (texel, shift) in texels.iter_mut().zip((0..32).step_by(2)) {
        let code = usize::try_from((indices >> shift) & 0x3).unwrap_or_default();
        *texel = palette.get(code).copied().unwrap_or_default();
    }
    texels
}

fn colour_palette(c0: u16, c1: u16, bc1_mode: bool) -> [[u8; 4]; 4] {
    let first = rgb565(c0);
    let second = rgb565(c1);
    if bc1_mode && bc1_has_transparent_selector(c0, c1) {
        [
            first,
            second,
            mix(first, second, |a, b| (a + b).div_ceil(2)),
            [0; 4],
        ]
    } else {
        [
            first,
            second,
            mix(first, second, |a, b| (2 * a + b + 1) / 3),
            mix(first, second, |a, b| (a + 2 * b + 1) / 3),
        ]
    }
}

fn rgb565(colour: u16) -> [u8; 4] {
    let expand = |value: u16, bits: u32| {
        let value = u8::try_from(value).unwrap_or_default();
        (value << (8 - bits)) | (value >> (2 * bits - 8))
    };
    [
        expand(colour >> 11, 5),
        expand((colour >> 5) & 0x3F, 6),
        expand(colour & 0x1F, 5),
        255,
    ]
}

fn mix(first: [u8; 4], second: [u8; 4], blend: impl Fn(u16, u16) -> u16) -> [u8; 4] {
    let channel = |a: u8, b: u8| u8::try_from(blend(u16::from(a), u16::from(b))).unwrap_or(u8::MAX);
    [
        channel(first[0], second[0]),
        channel(first[1], second[1]),
        channel(first[2], second[2]),
        255,
    ]
}

pub(crate) fn alpha_palette(a0: u8, a1: u8) -> [u8; 8] {
    let (a0, a1) = (u16::from(a0), u16::from(a1));
    let blend = |weight0: u16, weight1: u16, steps: u16| {
        u8::try_from((weight0 * a0 + weight1 * a1 + steps / 2) / steps).unwrap_or(u8::MAX)
    };
    let mut palette = [0; 8];
    for (code, value) in (0_u16..).zip(palette.iter_mut()) {
        *value = match code {
            0 => blend(1, 0, 1),
            1 => blend(0, 1, 1),
            _ if a0 > a1 => blend(8 - code, code - 1, 7),
            2..=5 => blend(6 - code, code - 1, 5),
            6 => 0,
            _ => 255,
        };
    }
    palette
}
