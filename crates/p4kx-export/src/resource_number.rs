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
#[non_exhaustive]
pub enum ResourceNumber {
    /// A source 8-bit signed integer.
    #[serde(rename = "int8")]
    Int8 {
        /// The stored integer.
        value: i8,
    },
    /// A source 16-bit signed integer.
    #[serde(rename = "int16")]
    Int16 {
        /// The stored integer.
        value: i16,
    },
    /// A source 32-bit signed integer.
    #[serde(rename = "int32")]
    Int32 {
        /// The stored integer.
        #[schemars(range(min = i32::MIN, max = i32::MAX))]
        value: i32,
    },
    /// A source 64-bit signed integer.
    #[serde(rename = "int64")]
    Int64 {
        /// The stored integer.
        value: ResourceInt64,
    },
    /// A source 8-bit unsigned integer.
    #[serde(rename = "uint8")]
    UInt8 {
        /// The stored integer.
        value: u8,
    },
    /// A source 16-bit unsigned integer.
    #[serde(rename = "uint16")]
    UInt16 {
        /// The stored integer.
        value: u16,
    },
    /// A source 32-bit unsigned integer.
    #[serde(rename = "uint32")]
    UInt32 {
        /// The stored integer.
        #[schemars(range(min = u32::MIN, max = u32::MAX))]
        value: u32,
    },
    /// A source 64-bit unsigned integer.
    #[serde(rename = "uint64")]
    UInt64 {
        /// The stored integer.
        value: DecimalU64,
    },
    /// A source IEEE 754 binary32 value.
    #[serde(rename = "f32")]
    Single(ResourceFloat32),
    /// A source IEEE 754 binary64 value.
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
