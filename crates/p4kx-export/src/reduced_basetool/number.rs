// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Serialize;

use crate::resource_number::ResourceNumber;

#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum Number<'source> {
    Integer(&'source ResourceNumber),
    Single(Float<f32>),
    Double(Float<f64>),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Float<T> {
    #[serde(rename = "type")]
    kind: FloatKind,
    finite: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    non_finite: Option<NonFinite>,
}

#[derive(Serialize)]
enum FloatKind {
    #[serde(rename = "f32")]
    Single,
    #[serde(rename = "f64")]
    Double,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum NonFinite {
    PositiveInfinity,
    NegativeInfinity,
    NaN,
}

impl<'source> From<&'source ResourceNumber> for Number<'source> {
    fn from(number: &'source ResourceNumber) -> Self {
        match number {
            ResourceNumber::Single(value) => Self::Single(Float {
                kind: FloatKind::Single,
                finite: value.finite(),
                non_finite: value
                    .finite()
                    .is_none()
                    .then(|| non_finite(f64::from(f32::from_bits(value.raw_bits())))),
            }),
            ResourceNumber::Double(value) => Self::Double(Float {
                kind: FloatKind::Double,
                finite: value.finite(),
                non_finite: value
                    .finite()
                    .is_none()
                    .then(|| non_finite(f64::from_bits(value.raw_bits()))),
            }),
            _ => Self::Integer(number),
        }
    }
}

fn non_finite(value: f64) -> NonFinite {
    if value.is_nan() {
        NonFinite::NaN
    } else if value.is_sign_negative() {
        NonFinite::NegativeInfinity
    } else {
        NonFinite::PositiveInfinity
    }
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::DecimalU64;
    use serde_json::json;

    use super::Number;
    use crate::resource_float::{ResourceFloat32, ResourceFloat64};
    use crate::resource_number::ResourceNumber;

    #[test]
    fn preserves_numeric_values_without_float_bits() -> Result<(), serde_json::Error> {
        let wide = ResourceNumber::UInt64 {
            value: DecimalU64::new(u64::MAX),
        };
        let finite = ResourceNumber::Single(ResourceFloat32::new(-0.0));
        let infinity = ResourceNumber::Double(ResourceFloat64::new(f64::NEG_INFINITY));
        assert_eq!(
            serde_json::to_value(Number::from(&wide))?,
            json!({"type":"uint64","value":"18446744073709551615"})
        );
        assert_eq!(
            serde_json::to_string(&Number::from(&finite))?,
            r#"{"type":"f32","finite":-0.0}"#
        );
        assert_eq!(
            serde_json::to_value(Number::from(&infinity))?,
            json!({"type":"f64","finite":null,"nonFinite":"negativeInfinity"})
        );
        Ok(())
    }
}
