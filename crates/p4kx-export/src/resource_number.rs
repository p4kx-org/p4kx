// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, Value};
use schemars::JsonSchema;
use serde::Serialize;

use crate::resource_float::{ResourceFloat32, ResourceFloat64};
use crate::resource_integer::ResourceInt64;

/// A numeric source value retaining its original integer width or IEEE bits.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "type")]
pub(crate) enum ResourceNumber {
    #[serde(rename = "int8")]
    Int8 { value: i8 },
    #[serde(rename = "int16")]
    Int16 { value: i16 },
    #[serde(rename = "int32")]
    Int32 { value: i32 },
    #[serde(rename = "int64")]
    Int64 { value: ResourceInt64 },
    #[serde(rename = "uint8")]
    UInt8 { value: u8 },
    #[serde(rename = "uint16")]
    UInt16 { value: u16 },
    #[serde(rename = "uint32")]
    UInt32 { value: u32 },
    #[serde(rename = "uint64")]
    UInt64 { value: DecimalU64 },
    #[serde(rename = "f32")]
    Single(ResourceFloat32),
    #[serde(rename = "f64")]
    Double(ResourceFloat64),
}

impl ResourceNumber {
    pub(crate) fn read(value: &Value<'_>) -> Option<Self> {
        match value {
            Value::Int8(value) => Some(Self::Int8 { value: *value }),
            Value::Int16(value) => Some(Self::Int16 { value: *value }),
            Value::Int32(value) => Some(Self::Int32 { value: *value }),
            Value::Int64(value) => Some(Self::Int64 {
                value: ResourceInt64::new(*value),
            }),
            Value::UInt8(value) => Some(Self::UInt8 { value: *value }),
            Value::UInt16(value) => Some(Self::UInt16 { value: *value }),
            Value::UInt32(value) => Some(Self::UInt32 { value: *value }),
            Value::UInt64(value) => Some(Self::UInt64 {
                value: DecimalU64::new(*value),
            }),
            Value::Single(value) => Some(Self::Single(ResourceFloat32::new(*value))),
            Value::Double(value) => Some(Self::Double(ResourceFloat64::new(*value))),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_width_and_wide_integer_digits_are_preserved() {
        for (value, kind, digits) in [
            (Value::Int64(i64::MIN), "int64", "-9223372036854775808"),
            (Value::Int64(i64::MAX), "int64", "9223372036854775807"),
            (Value::UInt64(u64::MAX), "uint64", "18446744073709551615"),
        ] {
            let number = ResourceNumber::read(&value).expect("numeric source fixture");
            let json = serde_json::to_value(number).expect("serialize exact integer fixture");
            assert_eq!(json["type"], kind);
            assert_eq!(json["value"], digits);
        }
        let number = ResourceNumber::read(&Value::Double(f64::from_bits(0x8000_0000_0000_0000)))
            .expect("source Double fixture");
        let json = serde_json::to_value(number).expect("serialize exact Double fixture");
        assert_eq!(json["type"], "f64");
        assert_eq!(json["rawBits"], "8000000000000000");
        assert_eq!(json["finite"].to_string(), "-0.0");
    }
}
