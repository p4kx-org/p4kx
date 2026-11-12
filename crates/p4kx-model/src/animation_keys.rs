// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::ParseError;

use crate::CorruptAnimation;

const HEADER_LEN: usize = 24;

const TIME_BYTES: u8 = 0x40;
const TIME_SHORTS: u8 = 0x41;
const TIME_BITMAP: u8 = 0x42;
const ROTATION_FLOAT: u8 = 0x80;
const ROTATION_SMALL_TREE_48: u8 = 0x82;
const ROTATION_SMALL_TREE_64: u8 = 0x83;
const POSITION_FLOAT: u8 = 0xC0;
const POSITION_INTERLEAVED: u8 = 0xC1;
const POSITION_PLANAR: u8 = 0xC2;

/// Returns the `len` bytes at `start`, or the truncation error.
pub(crate) fn bytes_at(data: &[u8], start: usize, len: usize) -> Result<&[u8], CorruptAnimation> {
    start
        .checked_add(len)
        .and_then(|end| data.get(start..end))
        .ok_or_else(|| {
            CorruptAnimation::Truncated(ParseError::UnexpectedEnd {
                offset: start as u64,
                needed: len as u64,
                available: data.len().saturating_sub(start) as u64,
            })
        })
}

pub(crate) struct EncodedValues<T> {
    pub(crate) values: Vec<T>,
    pub(crate) bytes: usize,
}

fn decode_values<T>(
    data: &[u8],
    start: usize,
    len: usize,
    decode: impl FnOnce(&[u8]) -> Vec<T>,
) -> Result<EncodedValues<T>, CorruptAnimation> {
    let bytes = bytes_at(data, start, len)?;
    Ok(EncodedValues {
        values: decode(bytes),
        bytes: bytes.len(),
    })
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    let pair = bytes
        .get(at..at + 2)
        .and_then(|pair| <[u8; 2]>::try_from(pair).ok());
    pair.map_or(0, u16::from_le_bytes)
}

fn f32s<const N: usize>(bytes: &[u8]) -> impl Iterator<Item = [f32; N]> {
    let words = bytes.as_chunks::<4>().0;
    words.as_chunks::<N>().0.iter().map(|chunk| {
        let mut out = [0.0; N];
        for (value, word) in out.iter_mut().zip(chunk) {
            *value = f32::from_le_bytes(*word);
        }
        out
    })
}

/// Reads the frames of `count` keys stored in format `code` at `start`.
pub(crate) fn read_times(
    data: &[u8],
    start: usize,
    code: u8,
    count: usize,
    controller_id: u32,
) -> Result<EncodedValues<u16>, CorruptAnimation> {
    let frames = match code {
        TIME_BYTES => decode_values(data, start, count, |bytes| {
            bytes.iter().map(|&frame| u16::from(frame)).collect()
        })?,
        TIME_SHORTS => decode_values(data, start, count.saturating_mul(2), |bytes| {
            bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&pair| u16::from_le_bytes(pair))
                .collect()
        })?,
        TIME_BITMAP => read_bitmap(data, start, count, controller_id)?,
        code => {
            return Err(CorruptAnimation::UnknownFormat { kind: "time", code });
        }
    };
    if frames
        .values
        .windows(2)
        .any(|pair| matches!(pair, [before, after] if before >= after))
    {
        return Err(CorruptAnimation::TimesNotIncreasing { controller_id });
    }
    Ok(frames)
}

fn read_bitmap(
    data: &[u8],
    start: usize,
    count: usize,
    controller_id: u32,
) -> Result<EncodedValues<u16>, CorruptAnimation> {
    let head = bytes_at(data, start, 4)?;
    let (first, last) = (u16_at(head, 0), u16_at(head, 2));
    if last < first {
        return Err(CorruptAnimation::BitmapRange {
            controller_id,
            first,
            last,
        });
    }
    let bits = usize::from(last - first) + 1;
    let bitmap = bytes_at(data, start + 4, bits.div_ceil(8))?;
    let mut frames = Vec::with_capacity(count);
    let mut marked = 0_usize;
    for (index, &byte) in bitmap.iter().enumerate() {
        let mut remaining = byte;
        while remaining != 0 {
            let bit = usize::try_from(remaining.trailing_zeros()).unwrap_or(0);
            remaining &= remaining - 1;
            let position = index * 8 + bit;
            if position < bits {
                marked += 1;
                if marked <= count {
                    frames.push(first + u16::try_from(position).unwrap_or(0));
                }
            }
        }
    }
    if marked != count {
        return Err(CorruptAnimation::BitmapCount {
            controller_id,
            expected: count,
            found: marked,
        });
    }
    Ok(EncodedValues {
        values: frames,
        bytes: head.len() + bitmap.len(),
    })
}

/// Reads `count` rotations stored in format `code` at `start` as unit quaternions in one
/// hemisphere.
pub(crate) fn read_rotations(
    data: &[u8],
    start: usize,
    code: u8,
    count: usize,
    controller_id: u32,
) -> Result<EncodedValues<[f32; 4]>, CorruptAnimation> {
    let mut rotations = match code {
        ROTATION_FLOAT => decode_values(data, start, count.saturating_mul(16), |bytes| {
            f32s::<4>(bytes).collect()
        })?,
        ROTATION_SMALL_TREE_48 => decode_values(data, start, count.saturating_mul(6), |bytes| {
            bytes
                .as_chunks::<6>()
                .0
                .iter()
                .map(|key| small_tree(key, [15, 15, 15], 48))
                .collect()
        })?,
        ROTATION_SMALL_TREE_64 => decode_values(data, start, count.saturating_mul(8), |bytes| {
            bytes
                .as_chunks::<8>()
                .0
                .iter()
                .map(|key| small_tree(key, [21, 21, 20], 64))
                .collect()
        })?,
        code => {
            return Err(CorruptAnimation::UnknownFormat {
                kind: "rotation",
                code,
            });
        }
    };
    let mut previous: Option<[f32; 4]> = None;
    for rotation in &mut rotations.values {
        if rotation.iter().any(|value| !value.is_finite()) {
            return Err(CorruptAnimation::NonFinite("rotation"));
        }
        let length = rotation
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();
        if !length.is_finite() || length < 1e-6 {
            return Err(CorruptAnimation::ZeroRotation { controller_id });
        }
        *rotation = rotation.map(|value| value / length);
        if let Some(before) = previous
            && before
                .iter()
                .zip(rotation.iter())
                .map(|(a, b)| a * b)
                .sum::<f32>()
                < 0.0
        {
            *rotation = rotation.map(|value| -value);
        }
        previous = Some(*rotation);
    }
    Ok(rotations)
}

/// Decodes a small-tree quaternion stored in the low `bits` bits of `bytes`: three components of
/// `widths` bits from bit 0 up, and the index of the omitted component in the top two of the
/// `bits` bits. The components cover ±1/√2 and run in ascending order of the quaternion's
/// `x, y, z, w` without the omitted one; the bits between the components and the index are
/// unused.
fn small_tree(bytes: &[u8], widths: [u32; 3], bits: u32) -> [f32; 4] {
    let mut word = [0_u8; 8];
    for (target, source) in word.iter_mut().zip(bytes) {
        *target = *source;
    }
    let raw = u64::from_le_bytes(word);
    let omitted = usize::try_from((raw >> (bits - 2)) & 3).unwrap_or(0);
    let mut quaternion = [0.0_f32; 4];
    let mut sum = 0.0_f32;
    let mut shift = 0;
    let kept = (0..4).filter(|&index| index != omitted);
    for (index, width) in kept.zip(widths) {
        let top = (1_u64 << width) - 1;
        let step = u32::try_from((raw >> shift) & top).unwrap_or(0);
        shift += width;
        #[expect(
            clippy::cast_precision_loss,
            reason = "a component has at most 21 bits, which an f32 holds exactly"
        )]
        let value = (step as f32 / top as f32 - 0.5) * std::f32::consts::SQRT_2;
        sum = value.mul_add(value, sum);
        if let Some(target) = quaternion.get_mut(index) {
            *target = value;
        }
    }
    if let Some(target) = quaternion.get_mut(omitted) {
        *target = (1.0 - sum).max(0.0).sqrt();
    }
    quaternion
}
/// Reads `count` translations stored in format `code` at `start`.
pub(crate) fn read_translations(
    data: &[u8],
    start: usize,
    code: u8,
    count: usize,
) -> Result<EncodedValues<[f32; 3]>, CorruptAnimation> {
    let translations = match code {
        POSITION_FLOAT => decode_values(data, start, count.saturating_mul(12), |bytes| {
            f32s::<3>(bytes).collect()
        })?,
        POSITION_INTERLEAVED | POSITION_PLANAR => {
            let header = bytes_at(data, start, HEADER_LEN)?;
            let mut axes = f32s::<3>(header);
            let scale = axes.next().unwrap_or_default();
            let offset = axes.next().unwrap_or_default();
            let body = start + HEADER_LEN;
            let mut decoded = if code == POSITION_INTERLEAVED {
                interleaved(data, body, count, scale, offset)?
            } else {
                planar(data, body, count, scale, offset)?
            };
            decoded.bytes += header.len();
            decoded
        }
        code => {
            return Err(CorruptAnimation::UnknownFormat {
                kind: "position",
                code,
            });
        }
    };
    if translations
        .values
        .iter()
        .flatten()
        .any(|value| !value.is_finite())
    {
        return Err(CorruptAnimation::NonFinite("position"));
    }
    Ok(translations)
}

fn active(scale: f32) -> bool {
    scale < f32::MAX
}

fn interleaved(
    data: &[u8],
    start: usize,
    count: usize,
    scale: [f32; 3],
    offset: [f32; 3],
) -> Result<EncodedValues<[f32; 3]>, CorruptAnimation> {
    decode_values(data, start, count.saturating_mul(6), |body| {
        body.as_chunks::<6>()
            .0
            .iter()
            .map(|key| {
                std::array::from_fn(|axis| {
                    let scale = scale.get(axis).copied().unwrap_or(0.0);
                    let offset = offset.get(axis).copied().unwrap_or(0.0);
                    if active(scale) {
                        f32::from(u16_at(key.as_slice(), axis * 2)).mul_add(scale, offset)
                    } else {
                        offset
                    }
                })
            })
            .collect()
    })
}

fn planar(
    data: &[u8],
    start: usize,
    count: usize,
    scale: [f32; 3],
    offset: [f32; 3],
) -> Result<EncodedValues<[f32; 3]>, CorruptAnimation> {
    let used = scale.iter().filter(|&&scale| active(scale)).count();
    let body = bytes_at(data, start, count.saturating_mul(2).saturating_mul(used))?;
    let mut translations = vec![offset; count];
    let mut column = 0;
    for axis in 0..3 {
        if !scale.get(axis).is_some_and(|&scale| active(scale)) {
            continue;
        }
        let scale = scale.get(axis).copied().unwrap_or(0.0);
        let values = body.get(column * count * 2..(column + 1) * count * 2);
        column += 1;
        for (key, step) in translations
            .iter_mut()
            .zip(values.unwrap_or_default().as_chunks::<2>().0)
        {
            let base = key.get(axis).copied().unwrap_or(0.0);
            if let Some(target) = key.get_mut(axis) {
                *target = f32::from(u16_at(step, 0)).mul_add(scale, base);
            }
        }
    }
    Ok(EncodedValues {
        values: translations,
        bytes: body.len(),
    })
}
