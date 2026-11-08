// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Value as Raw, ValueError};
use serde::Serialize;
use serde_json::Value;

use super::{Failure, Result};

pub(super) fn state(carrier: &Value, expected: &str) -> bool {
    carrier["state"] == expected && carrier.get("value").is_some_and(Value::is_null)
}

pub(super) fn present(carrier: &Value) -> Option<&Value> {
    if carrier["state"] == "present" {
        carrier.get("value")
    } else {
        None
    }
}

pub(super) fn scalar(raw: &Raw<'_>, value: &Value, path: &str) -> Result<bool> {
    match raw {
        Raw::Bool(expected) => Ok(value.as_bool() == Some(*expected)),
        Raw::String(expected) | Raw::Locale(expected) | Raw::Enum(expected) => {
            Ok(value.as_str() == Some(*expected))
        }
        Raw::Guid(expected) => Ok(value.as_str() == Some(expected.to_string().as_str())),
        _ => number(raw, value, path),
    }
}

fn number(raw: &Raw<'_>, value: &Value, path: &str) -> Result<bool> {
    let equal = match raw {
        Raw::Int8(expected) => signed(value, "int8", i64::from(*expected), false),
        Raw::Int16(expected) => signed(value, "int16", i64::from(*expected), false),
        Raw::Int32(expected) => signed(value, "int32", i64::from(*expected), plain_int32(path)),
        Raw::Int64(expected) => {
            value["type"] == "int64"
                && value["value"].as_str() == Some(expected.to_string().as_str())
        }
        Raw::UInt8(expected) => unsigned(value, "uint8", u64::from(*expected)),
        Raw::UInt16(expected) => unsigned(value, "uint16", u64::from(*expected)),
        Raw::UInt32(expected) => unsigned(value, "uint32", u64::from(*expected)),
        Raw::UInt64(expected) => {
            value["type"] == "uint64"
                && value["value"].as_str() == Some(expected.to_string().as_str())
        }
        Raw::Single(expected) => {
            return float(
                value,
                &format!("{:08x}", expected.to_bits()),
                *expected,
                "f32",
                path,
            );
        }
        Raw::Double(expected) => {
            return float(
                value,
                &format!("{:016x}", expected.to_bits()),
                *expected,
                "f64",
                path,
            );
        }
        _ => false,
    };
    Ok(equal)
}

fn plain_int32(path: &str) -> bool {
    matches!(
        path.rsplit('.').next(),
        Some("min" | "max" | "start" | "end" | "mappedValue")
    )
}
fn signed(value: &Value, kind: &str, expected: i64, plain: bool) -> bool {
    if plain {
        return value.as_i64() == Some(expected);
    }
    value["type"] == kind && value["value"].as_i64() == Some(expected)
}

fn unsigned(value: &Value, kind: &str, expected: u64) -> bool {
    value["type"] == kind && value["value"].as_u64() == Some(expected)
}

fn float<T: Serialize>(
    value: &Value,
    bits: &str,
    source: T,
    kind: &str,
    path: &str,
) -> Result<bool> {
    let bytes =
        serde_json::to_vec(&source).map_err(|error| Failure::source("raw-float-json", error))?;
    let expected: Value = serde_json::from_slice(&bytes)
        .map_err(|error| Failure::source("raw-float-number", error))?;
    let tagged = matches!(path.rsplit('.').next(), Some("mean" | "stddev"));
    let width = if tagged {
        value["type"] == kind
    } else {
        value.get("type").is_none()
    };
    let finite = value.get("finite").is_some_and(|actual| {
        actual == &expected
            && actual.as_f64().map(f64::to_bits) == expected.as_f64().map(f64::to_bits)
    });
    Ok(width && value["rawBits"].as_str() == Some(bits) && finite)
}
pub(super) fn reference(raw: &Raw<'_>, value: &Value) -> bool {
    match raw {
        Raw::Reference(target) if target.location().is_none() => {
            value["status"] == "present"
                && present(&value["guid"]).and_then(Value::as_str)
                    == Some(target.guid().to_string().as_str())
                && present(&value["targetRecordIndex"]).and_then(Value::as_u64)
                    == Some(u64::from(target.record_index()))
                && present(&value["recordPath"]).and_then(Value::as_str)
                    == Some(target.record_path())
                && present(&value["targetType"]).and_then(Value::as_str) == Some(target.type_name())
        }
        Raw::Error(ValueError::DanglingReference { guid }) => {
            unresolved(value, &guid.to_string(), "dangling")
        }
        Raw::Error(ValueError::UnexpectedReferenceType { guid, .. }) => {
            unresolved(value, &guid.to_string(), "unexpectedTargetType")
        }
        _ => false,
    }
}

fn unresolved(value: &Value, guid: &str, status: &str) -> bool {
    value["status"] == status
        && present(&value["guid"]).and_then(Value::as_str) == Some(guid)
        && ["targetRecordIndex", "recordPath", "targetType"]
            .iter()
            .all(|field| state(&value[*field], "absent"))
}

pub(super) const fn is_reference(raw: &Raw<'_>) -> bool {
    matches!(
        raw,
        Raw::Reference(_)
            | Raw::Error(
                ValueError::DanglingReference { .. } | ValueError::UnexpectedReferenceType { .. }
            )
    )
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::{Guid, ValueError};
    use serde_json::json;

    use super::{Raw, Result, reference, scalar, state};

    #[test]
    fn source_float_bits_preserve_nan_payloads_and_numeric_width() -> Result<()> {
        let raw = Raw::Single(f32::from_bits(0x7fc0_0017));
        assert!(
            scalar(
                &raw,
                &json!({"rawBits": "7fc00017", "finite": null}),
                "properties[0].volatility"
            )?,
            "NaN bits survive"
        );
        assert!(
            !scalar(
                &raw,
                &json!({"rawBits": "7fc00018", "finite": null}),
                "properties[0].volatility"
            )?,
            "the payload is exact"
        );
        assert!(
            !scalar(
                &raw,
                &json!({"rawBits": "7fc00017", "finite": 0}),
                "properties[0].volatility"
            )?,
            "nonfinite has no decimal value"
        );
        assert!(
            !scalar(
                &raw,
                &json!({"type": "f64", "rawBits": "7fc00017", "finite": null}),
                "properties[0].mean"
            )?,
            "numeric width is retained"
        );
        Ok(())
    }

    #[test]
    fn finite_projection_matches_the_stored_float_without_widening_roundoff() -> Result<()> {
        let source = 1.23_f32;
        let bits = format!("{:08x}", source.to_bits());
        assert!(
            scalar(
                &Raw::Single(source),
                &json!({"rawBits": bits, "finite": 1.23}),
                "properties[0].volatility"
            )?,
            "the decimal round-trips the Single"
        );
        assert!(
            !scalar(
                &Raw::Single(source),
                &json!({"rawBits": bits, "finite": 999}),
                "properties[0].volatility"
            )?,
            "correct bits cannot hide a wrong decimal"
        );
        assert!(
            !scalar(
                &Raw::Single(-0.0),
                &json!({"rawBits": "80000000", "finite": 0.0}),
                "properties[0].volatility"
            )?,
            "decimal zero retains its sign"
        );
        assert!(
            scalar(
                &Raw::Single(-0.0),
                &json!({"rawBits": "80000000", "finite": -0.0}),
                "properties[0].volatility"
            )?,
            "negative zero round-trips"
        );
        Ok(())
    }

    #[test]
    fn wide_integers_use_exact_decimal_strings() -> Result<()> {
        assert!(
            scalar(
                &Raw::UInt64(u64::MAX),
                &json!({"type": "uint64", "value": u64::MAX.to_string()}),
                "properties[0].mean"
            )?,
            "wide integers retain every bit"
        );
        assert!(
            !scalar(
                &Raw::UInt64(u64::MAX),
                &json!({"type": "uint64", "value": u64::MAX}),
                "properties[0].mean"
            )?,
            "a JSON number cannot replace the decimal string"
        );
        Ok(())
    }

    #[test]
    fn false_zero_null_and_absence_are_distinct() -> Result<()> {
        assert!(
            scalar(&Raw::Bool(false), &json!(false), "validateDefaultCargoBox")?,
            "false is a stored value"
        );
        assert!(
            !scalar(&Raw::Bool(false), &json!(0), "validateDefaultCargoBox")?,
            "false is not numeric zero"
        );
        assert!(
            scalar(&Raw::Int32(0), &json!(0), "quality.min")?,
            "zero is an authored bound"
        );
        assert!(
            !scalar(&Raw::Int32(0), &json!(0), "quality.mean")?,
            "a generic numeric source retains its width"
        );
        assert!(
            state(&json!({"state": "null", "value": null}), "null"),
            "null is explicit"
        );
        assert!(
            !state(&json!({"state": "absent", "value": null}), "null"),
            "absence is separate"
        );
        assert!(
            !state(&json!({"state": "null", "value": false}), "null"),
            "null has no boolean payload"
        );
        assert!(
            !state(&json!({"state": "null"}), "null"),
            "null retains its required value key"
        );
        Ok(())
    }

    #[test]
    fn unresolved_references_preserve_authored_guids_and_absent_target_headers() {
        let guid = Guid::from_stored([9; 16]);
        let raw = Raw::Error(ValueError::DanglingReference { guid });
        let absent = json!({"state": "absent", "value": null});
        let mut value = json!({"status": "dangling",
            "guid": {"state": "present", "value": guid.to_string()},
            "targetRecordIndex": absent, "recordPath": absent, "targetType": absent});
        assert!(
            reference(&raw, &value),
            "the declared GUID survives without a target"
        );
        value["status"] = json!("present");
        assert!(
            !reference(&raw, &value),
            "an unresolved box cannot claim a resolved target"
        );
        value["status"] = json!("dangling");
        value["recordPath"]["state"] = json!("null");
        assert!(
            !reference(&raw, &value),
            "unavailable target metadata remains absent"
        );
    }
}
