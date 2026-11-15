// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::hirc_rtpc::{self, BankRtpcExtent, CURVE_POINT_LEN};
use crate::{BankError, SectionId};

pub(crate) const VERSION: u32 = 154;
const SELECTORS: usize = 19;
const CONE_ENABLED_BIT: u8 = 1;

/// Original attenuation cone words and their bank-relative byte ranges.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankAttenuationCone {
    /// IEEE-754 word of the inside angle in degrees.
    pub inside_degrees_bits: Option<BankHircField<u32>>,
    /// IEEE-754 word of the outside angle in degrees.
    pub outside_degrees_bits: Option<BankHircField<u32>>,
    /// Original outside volume word.
    pub outside_volume_bits: Option<BankHircField<u32>>,
    /// Original low-pass word.
    pub low_pass_bits: Option<BankHircField<u32>>,
    /// Original high-pass word.
    pub high_pass_bits: Option<BankHircField<u32>>,
}

/// One physical conversion curve and its admitted complete point prefix.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankAttenuationCurve {
    /// Original scaling code.
    pub scaling: BankHircField<u8>,
    /// Original number of graph points.
    pub point_count: Option<BankHircField<u16>>,
    /// Original extent of the complete graph points.
    pub points_range: Range<u64>,
}

/// Original values admitted before a complete or malformed attenuation layout ends.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankAttenuationFields {
    /// Original height-spread flag byte.
    pub height_spread_flags: Option<BankHircField<u8>>,
    /// Original cone flag byte.
    pub cone_flags: Option<BankHircField<u8>>,
    /// Original cone words when the enabled bit is set.
    pub cone: Option<BankAttenuationCone>,
    /// Signed selectors in physical slot order.
    pub selectors: Vec<BankHircField<i8>>,
    /// Original conversion-curve count.
    pub curve_count: Option<BankHircField<u8>>,
    /// Original conversion curves in physical order.
    pub curves: Vec<BankAttenuationCurve>,
    /// Original initial RTPC count and complete curve prefix.
    pub rtpc: Option<BankRtpcExtent>,
    pub(crate) checked_end: u64,
}

impl BankAttenuationFields {
    /// Tests the original cone-enable bit when the flags are available.
    #[must_use]
    pub fn cone_enabled(&self) -> Option<bool> {
        Some(enabled(self.cone_flags.as_ref()?.value))
    }
}

const fn enabled(flags: u8) -> bool {
    flags & CONE_ENABLED_BIT != 0
}

pub(crate) fn scan<F: ObjectFields>(
    input: &mut F,
    fields: &mut BankAttenuationFields,
) -> Result<(), F::Error> {
    fields.checked_end = input.offset();
    fields.height_spread_flags = Some(input.read_u8()?);
    fields.checked_end = input.offset();
    let flags = input.read_u8()?;
    if enabled(flags.value) {
        fields.cone = Some(BankAttenuationCone::default());
    }
    fields.cone_flags = Some(flags);
    fields.checked_end = input.offset();
    if let Some(cone) = &mut fields.cone {
        scan_cone(input, cone, &mut fields.checked_end)?;
    }
    for _ in 0..SELECTORS {
        fields.selectors.push(input.scalar(i8::from_le_bytes)?);
        fields.checked_end = input.offset();
    }

    let count = input.read_u8()?;
    let curves = count.value;
    fields.curve_count = Some(count);
    fields.checked_end = input.offset();
    for _ in 0..curves {
        scan_curve(input, fields)?;
    }
    let rtpc = hirc_rtpc::scan(input, &mut fields.rtpc);
    if let Some(extent) = &fields.rtpc {
        fields.checked_end = extent.checked_range.end;
    }
    rtpc?;
    if input.remaining_bytes() != 0 {
        return Err(BankError::invalid(
            SectionId::HIRC,
            "bytes follow the attenuation RTPC curves",
        )
        .into());
    }
    Ok(())
}

fn scan_cone<F: ObjectFields>(
    input: &mut F,
    cone: &mut BankAttenuationCone,
    checked_end: &mut u64,
) -> Result<(), F::Error> {
    for field in [
        &mut cone.inside_degrees_bits,
        &mut cone.outside_degrees_bits,
        &mut cone.outside_volume_bits,
        &mut cone.low_pass_bits,
        &mut cone.high_pass_bits,
    ] {
        *field = Some(input.read_u32()?);
        *checked_end = input.offset();
    }
    Ok(())
}

fn scan_curve<F: ObjectFields>(
    input: &mut F,
    fields: &mut BankAttenuationFields,
) -> Result<(), F::Error> {
    let scaling = input.read_u8()?;
    fields.checked_end = input.offset();
    let mut curve = BankAttenuationCurve {
        scaling,
        point_count: None,
        points_range: fields.checked_end..fields.checked_end,
    };
    let count = match input.read_u16() {
        Ok(count) => count,
        Err(error) => {
            fields.curves.push(curve);
            return Err(error);
        }
    };
    let bytes = u64::from(count.value) * CURVE_POINT_LEN as u64;
    let start = input.offset();
    let available = input.remaining_bytes() / CURVE_POINT_LEN as u64 * CURVE_POINT_LEN as u64;
    let admitted = bytes.min(available);
    curve.point_count = Some(count);
    curve.points_range = start..start + admitted;
    fields.checked_end = curve.points_range.end;
    fields.curves.push(curve);
    input.skip_bytes(bytes)
}
