// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value as Raw;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::TestResult;
use crate::gameplay_v3_original_origins as original;

pub(crate) fn check(carrier: &Value, raw: Option<&Raw<'_>>) -> TestResult {
    match raw {
        None => {
            original::state(carrier, "absent")?;
        }
        Some(Raw::Null) => {
            original::state(carrier, "null")?;
        }
        Some(Raw::Error(_)) => {
            original::state(carrier, "unreadable")?;
        }
        Some(Raw::Single(value)) => single(carrier, *value)?,
        Some(Raw::Double(value)) => double(carrier, *value)?,
        Some(Raw::Int8(value)) => integer(carrier, "int8", &json!(value))?,
        Some(Raw::Int16(value)) => integer(carrier, "int16", &json!(value))?,
        Some(Raw::Int32(value)) => integer(carrier, "int32", &json!(value))?,
        Some(Raw::Int64(value)) => integer(carrier, "int64", &json!(value.to_string()))?,
        Some(Raw::UInt8(value)) => integer(carrier, "uint8", &json!(value))?,
        Some(Raw::UInt16(value)) => integer(carrier, "uint16", &json!(value))?,
        Some(Raw::UInt32(value)) => integer(carrier, "uint32", &json!(value))?,
        Some(Raw::UInt64(value)) => integer(carrier, "uint64", &json!(value.to_string()))?,
        Some(_) => {
            original::state(carrier, "invalid")?;
        }
    }
    Ok(())
}

fn number<'a>(carrier: &'a Value, tag: &str) -> TestResult<&'a Value> {
    let value = original::state(carrier, "present")?.ok_or("original numeric payload")?;
    if value.get("type").and_then(Value::as_str) != Some(tag) {
        return Err("original numeric width differs".into());
    }
    Ok(value)
}

fn integer(carrier: &Value, tag: &str, expected: &Value) -> TestResult {
    if number(carrier, tag)?.get("value") != Some(expected) {
        return Err("original integer value differs".into());
    }
    Ok(())
}

fn single(carrier: &Value, original: f32) -> TestResult {
    let value = number(carrier, "f32")?;
    if value.get("rawBits").and_then(Value::as_str)
        != Some(format!("{:08x}", original.to_bits()).as_str())
    {
        return Err("original Single bits differ".into());
    }
    let finite = value.get("finite").ok_or("Single finite representation")?;
    if original.is_finite() {
        if !finite.is_number() || finite.to_string().parse::<f32>()?.to_bits() != original.to_bits()
        {
            return Err("original finite Single does not round trip".into());
        }
    } else if !finite.is_null() {
        return Err("a nonfinite original Single cannot invent a finite value".into());
    }
    Ok(())
}

fn double(carrier: &Value, original: f64) -> TestResult {
    let value = number(carrier, "f64")?;
    if value.get("rawBits").and_then(Value::as_str)
        != Some(format!("{:016x}", original.to_bits()).as_str())
    {
        return Err("original Double bits differ".into());
    }
    let finite = value.get("finite").ok_or("Double finite representation")?;
    if original.is_finite() {
        if finite.as_f64().map(f64::to_bits) != Some(original.to_bits()) {
            return Err("original finite Double does not round trip".into());
        }
    } else if !finite.is_null() {
        return Err("a nonfinite original Double cannot invent a finite value".into());
    }
    Ok(())
}
