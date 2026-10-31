// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{Reference, StructValue, Value};

/// The largest integer that an `f64` holds exactly.
const EXACT_INTEGER: i64 = 1 << 53;

/// Reads a number value of any width as an `f64`, an integer exactly.
///
/// An integer beyond 2^53 and any other kind of value give `None`, so that no value is rounded
/// or read as zero without a word.
#[expect(
    clippy::cast_precision_loss,
    reason = "a 64-bit integer is checked to be at most 2^53 in magnitude"
)]
pub(crate) fn exact(value: &Value<'_>) -> Option<f64> {
    match value {
        Value::Single(value) => Some(f64::from(*value)),
        Value::Double(value) => Some(*value),
        Value::Int32(value) => Some(f64::from(*value)),
        Value::UInt32(value) => Some(f64::from(*value)),
        Value::Int16(value) => Some(f64::from(*value)),
        Value::UInt16(value) => Some(f64::from(*value)),
        Value::Int8(value) => Some(f64::from(*value)),
        Value::UInt8(value) => Some(f64::from(*value)),
        Value::Int64(value) => {
            (value.unsigned_abs() <= EXACT_INTEGER.unsigned_abs()).then_some(*value as f64)
        }
        Value::UInt64(value) => (i64::try_from(*value).is_ok_and(|value| value <= EXACT_INTEGER))
            .then_some(*value as f64),
        _ => None,
    }
}

/// Reads a number field as a single-precision float, the precision of the layout.
///
/// `None` when the field is absent or holds no number that [`exact`] reads.
#[expect(
    clippy::cast_possible_truncation,
    reason = "layout values are single-precision floats"
)]
pub(crate) fn float(structure: &StructValue<'_>, name: &str) -> Option<f32> {
    exact(structure.field(name)?).map(|value| value as f32)
}

/// Reads an integer field that fits `i32`; wider values give `None`, unlike [`exact`], which
/// takes integers up to 2^53.
pub(crate) fn int(structure: &StructValue<'_>, name: &str) -> Option<i32> {
    match structure.field(name)? {
        Value::Int32(value) => Some(*value),
        Value::Int16(value) => Some(i32::from(*value)),
        Value::Int8(value) => Some(i32::from(*value)),
        Value::UInt16(value) => Some(i32::from(*value)),
        Value::UInt8(value) => Some(i32::from(*value)),
        Value::UInt32(value) => i32::try_from(*value).ok(),
        Value::Int64(value) => i32::try_from(*value).ok(),
        Value::UInt64(value) => i32::try_from(*value).ok(),
        _ => None,
    }
}

pub(crate) fn text<'a>(structure: &'a StructValue<'_>, name: &str) -> Option<&'a str> {
    match structure.field(name)? {
        Value::String(text) | Value::Enum(text) | Value::Locale(text) => Some(text),
        _ => None,
    }
}

pub(crate) fn flag(structure: &StructValue<'_>, name: &str) -> Option<bool> {
    match structure.field(name)? {
        Value::Bool(flag) => Some(*flag),
        _ => None,
    }
}

pub(crate) fn child<'a, 'f>(
    structure: &'a StructValue<'f>,
    name: &str,
) -> Option<&'a StructValue<'f>> {
    match structure.field(name)? {
        Value::Struct(child) => Some(child),
        _ => None,
    }
}

pub(crate) fn items<'a, 'f>(structure: &'a StructValue<'f>, name: &str) -> &'a [Value<'f>] {
    match structure.field(name) {
        Some(Value::Array(items)) => items,
        _ => &[],
    }
}

pub(crate) fn reference<'a, 'f>(
    structure: &'a StructValue<'f>,
    name: &str,
) -> Option<&'a Reference<'f>> {
    match structure.field(name)? {
        Value::Reference(reference) => Some(reference),
        _ => None,
    }
}

pub(crate) const fn as_struct<'a, 'f>(value: &'a Value<'f>) -> Option<&'a StructValue<'f>> {
    match value {
        Value::Struct(structure) => Some(structure),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_integer_width_is_read_exactly() {
        let big = i64::from(i32::MAX) + 1;
        assert_eq!(exact(&Value::Int8(-8)), Some(-8.0));
        assert_eq!(exact(&Value::UInt8(250)), Some(250.0));
        assert_eq!(exact(&Value::Int16(-300)), Some(-300.0));
        assert_eq!(exact(&Value::UInt16(60_000)), Some(60_000.0));
        assert_eq!(exact(&Value::Int32(i32::MIN)), Some(f64::from(i32::MIN)));
        assert_eq!(exact(&Value::UInt32(u32::MAX)), Some(f64::from(u32::MAX)));
        assert_eq!(exact(&Value::Int64(4)), Some(4.0));
        assert_eq!(exact(&Value::Int64(big)), Some(2_147_483_648.0));
        assert_eq!(exact(&Value::UInt64(7)), Some(7.0));
        assert_eq!(exact(&Value::UInt64(1 << 40)), Some(1_099_511_627_776.0));
    }

    #[test]
    fn a_64_bit_integer_beyond_exact_range_is_not_read() {
        assert_eq!(exact(&Value::Int64(EXACT_INTEGER + 1)), None);
        assert_eq!(exact(&Value::Int64(-EXACT_INTEGER - 1)), None);
        assert_eq!(exact(&Value::UInt64(u64::MAX)), None);
        assert_eq!(exact(&Value::Bool(true)), None);
    }

    #[test]
    fn floats_keep_their_value() {
        assert_eq!(exact(&Value::Single(0.5)), Some(0.5));
        assert_eq!(exact(&Value::Double(0.25)), Some(0.25));
    }
}
