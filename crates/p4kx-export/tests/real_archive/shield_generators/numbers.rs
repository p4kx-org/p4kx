// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value as Original;
use serde::Serialize;
use serde_json::{Value, json};

use super::TestResult;

pub(super) fn expected(raw: &Original<'_>) -> TestResult<Option<Value>> {
    let (kind, value) = match raw {
        Original::Int8(value) => ("int8", json!(value)),
        Original::Int16(value) => ("int16", json!(value)),
        Original::Int32(value) => ("int32", json!(value)),
        Original::Int64(value) => ("int64", json!(value.to_string())),
        Original::UInt8(value) => ("uint8", json!(value)),
        Original::UInt16(value) => ("uint16", json!(value)),
        Original::UInt32(value) => ("uint32", json!(value)),
        Original::UInt64(value) => ("uint64", json!(value.to_string())),
        Original::Single(value) => {
            return float("f32", &format!("{:08x}", value.to_bits()), value).map(Some);
        }
        Original::Double(value) => {
            return float("f64", &format!("{:016x}", value.to_bits()), value).map(Some);
        }
        _ => return Ok(None),
    };
    Ok(Some(json!({"type": kind, "value": value})))
}

fn float<T: Serialize>(kind: &str, bits: &str, value: &T) -> TestResult<Value> {
    let json = serde_json::to_vec(value).map_err(|_| "serialize original float")?;
    let finite: Value =
        serde_json::from_slice(&json).map_err(|_| "decode original float projection")?;
    Ok(json!({"type": kind, "rawBits": bits, "finite": finite}))
}

pub(super) fn equal(expected: &Value, actual: &Value) -> bool {
    let finite_bits = |value: &Value| {
        value
            .get("finite")
            .and_then(Value::as_f64)
            .map(f64::to_bits)
    };
    expected == actual && finite_bits(expected) == finite_bits(actual)
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::Value as Original;
    use serde_json::json;

    use super::{TestResult, equal, expected};

    #[test]
    fn shield_witness_distinguishes_numeric_width_and_float_sign() -> TestResult {
        let int = expected(&Original::Int8(3))?.ok_or("integer projection")?;
        assert!(
            !equal(&int, &json!({"type":"int16","value":3})),
            "integer width differs"
        );
        let zero = expected(&Original::Single(-0.0))?.ok_or("negative zero projection")?;
        assert!(
            equal(
                &zero,
                &json!({"type":"f32","rawBits":"80000000","finite":-0.0})
            ),
            "negative zero agrees"
        );
        assert!(
            !equal(
                &zero,
                &json!({"type":"f32","rawBits":"80000000","finite":0.0})
            ),
            "positive zero differs"
        );
        let nan = expected(&Original::Double(f64::from_bits(0x7ff0_0000_0000_0001)))?
            .ok_or("NaN projection")?;
        assert!(
            equal(
                &nan,
                &json!({"type":"f64","rawBits":"7ff0000000000001","finite":null})
            ),
            "nonfinite projection retains its payload bits"
        );
        Ok(())
    }
}
