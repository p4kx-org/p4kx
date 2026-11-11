// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value};

use crate::record_fields;

const I64_END: f64 = 9_223_372_036_854_775_808.0;

const SECONDS_PER_MINUTE: i64 = 60;
const SECONDS_PER_HOUR: i64 = 3_600;
const SECONDS_PER_DAY: i64 = 86_400;

pub(crate) const fn as_struct<'t, 'f>(value: &'t Value<'f>) -> Option<&'t StructValue<'f>> {
    match value {
        Value::Struct(value) => Some(value),
        _ => None,
    }
}

pub(crate) fn craft_time(time: Option<&StructValue<'_>>) -> Option<i64> {
    let part = |name: &str, factor: i64| whole(time, name)?.checked_mul(factor);
    let total = part("days", SECONDS_PER_DAY)?
        .checked_add(part("hours", SECONDS_PER_HOUR)?)?
        .checked_add(part("minutes", SECONDS_PER_MINUTE)?)?
        .checked_add(part("seconds", 1)?)?;
    (total >= 0).then_some(total)
}

/// Returns the original integer or a float rounded to a representable whole number.
pub(crate) fn whole(parent: Option<&StructValue<'_>>, name: &str) -> Option<i64> {
    match parent?.field(name)? {
        Value::Single(value) => rounded_i64(f64::from(*value)),
        Value::Double(value) => rounded_i64(*value),
        value => record_fields::integer_value(value),
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the rounded value is within the exact i64 conversion interval"
)]
fn rounded_i64(value: f64) -> Option<i64> {
    let rounded = value.round();
    if !(-I64_END..I64_END).contains(&rounded) {
        return None;
    }
    Some(rounded as i64)
}

#[cfg(test)]
#[path = "blueprint_integer_tests.rs"]
mod integer_tests;
