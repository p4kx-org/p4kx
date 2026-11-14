// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::SectionId;
use crate::bank_object_fields::{BankHircField, ObjectFields};

pub(crate) const CURVE_POINT_LEN: usize = 3 * size_of::<u32>();

/// Original initial RTPC count and complete curve prefix.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct BankRtpcExtent {
    /// Original curve count.
    pub curve_count: BankHircField<u16>,
    /// Number of complete curves admitted by the original body.
    pub checked_curves: u16,
    /// Bank-relative count and complete curve ranges.
    pub checked_range: Range<u64>,
}

/// One authored initial RTPC curve header.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankRtpcCurve {
    /// Authored RTPC ID.
    pub rtpc_id: BankHircField<u32>,
    /// Original RTPC type byte.
    pub rtpc_type: BankHircField<u8>,
    /// Original accumulation byte.
    pub accumulation: BankHircField<u8>,
    /// Authored variable-length parameter ID.
    pub parameter_id: BankHircField<u32>,
    /// Authored curve ID.
    pub curve_id: BankHircField<u32>,
    /// Original scaling byte.
    pub scaling: BankHircField<u8>,
    /// Original count of points in this curve.
    pub point_count: BankHircField<u16>,
}

/// One original RTPC graph point.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankRtpcPoint {
    /// Unmodified IEEE-754 input word.
    pub from_bits: BankHircField<u32>,
    /// Unmodified IEEE-754 output word.
    pub to_bits: BankHircField<u32>,
    /// Original interpolation word, including unknown codes.
    pub interpolation: BankHircField<u32>,
}

impl BankRtpcPoint {
    /// Returns the input word interpreted as IEEE-754 binary32.
    #[must_use]
    pub const fn input(&self) -> f32 {
        f32::from_bits(self.from_bits.value)
    }

    /// Returns the output word interpreted as IEEE-754 binary32.
    #[must_use]
    pub const fn output(&self) -> f32 {
        f32::from_bits(self.to_bits.value)
    }
}

pub(crate) fn scan<F: ObjectFields>(
    fields: &mut F,
    observed: &mut Option<BankRtpcExtent>,
) -> Result<(), F::Error> {
    let count = fields.read_u16()?;
    let end = count.range.end;
    let curves = count.value;
    let observation = observed.insert(BankRtpcExtent {
        checked_range: count.range.start..end,
        curve_count: count,
        checked_curves: 0,
    });
    for checked in 1..=curves {
        let curve = read_curve(fields)?;
        let len = u64::from(curve.point_count.value) * CURVE_POINT_LEN as u64;
        fields.skip_bytes(len)?;
        observation.checked_curves = checked;
        observation.checked_range.end = fields.offset();
    }
    Ok(())
}

pub(crate) fn read_curve<F: ObjectFields>(fields: &mut F) -> Result<BankRtpcCurve, F::Error> {
    let start = fields.offset();
    let [a, b, c, d, kind, accum] = fields.read_array()?;
    let rtpc_id = sourced(u32::from_le_bytes([a, b, c, d]), start);
    let rtpc_type = sourced(kind, rtpc_id.range.end);
    let accumulation = sourced(accum, rtpc_type.range.end);
    let parameter_id = fields.read_variable_length(SectionId::HIRC)?;
    let start = fields.offset();
    let [a, b, c, d, scale, low, high] = fields.read_array()?;
    let curve_id = sourced(u32::from_le_bytes([a, b, c, d]), start);
    let scaling = sourced(scale, curve_id.range.end);
    let point_count = sourced(u16::from_le_bytes([low, high]), scaling.range.end);
    Ok(BankRtpcCurve {
        rtpc_id,
        rtpc_type,
        accumulation,
        parameter_id,
        curve_id,
        scaling,
        point_count,
    })
}

pub(crate) fn read_point<F: ObjectFields>(fields: &mut F) -> Result<BankRtpcPoint, F::Error> {
    let start = fields.offset();
    let bytes = fields.read_array::<CURVE_POINT_LEN>()?;
    let [input, output, mode] = bytes.as_chunks::<{ size_of::<u32>() }>().0 else {
        unreachable!("a point contains three 32-bit words");
    };
    let from_bits = sourced(u32::from_le_bytes(*input), start);
    let to_bits = sourced(u32::from_le_bytes(*output), from_bits.range.end);
    let interpolation = sourced(u32::from_le_bytes(*mode), to_bits.range.end);
    Ok(BankRtpcPoint {
        from_bits,
        to_bits,
        interpolation,
    })
}

const fn sourced<T>(value: T, start: u64) -> BankHircField<T> {
    BankHircField {
        value,
        range: start..start + size_of::<T>() as u64,
    }
}
