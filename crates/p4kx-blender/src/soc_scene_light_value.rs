// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, Guid, SourceValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub(crate) enum LightValue {
    Boolean(bool),
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(String),
    UInt8(u8),
    UInt16(u16),
    UInt32(u32),
    UInt64(DecimalU64),
    Single(Float32),
    Double(Float64),
    Text(String),
    Locale(String),
    Guid(String),
    Enum(String),
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Float32 {
    bits: u32,
    finite: Option<f32>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Float64 {
    bits: DecimalU64,
    finite: Option<f64>,
}

impl LightValue {
    pub(crate) fn class(value: &Value<'_>) -> Option<SourceValue<Self>> {
        let value = match value {
            Value::Bool(value) => Self::Boolean(*value),
            Value::Int8(value) => Self::Int8(*value),
            Value::Int16(value) => Self::Int16(*value),
            Value::Int32(value) => Self::Int32(*value),
            Value::Int64(value) => Self::Int64(value.to_string()),
            Value::UInt8(value) => Self::UInt8(*value),
            Value::UInt16(value) => Self::UInt16(*value),
            Value::UInt32(value) => Self::UInt32(*value),
            Value::UInt64(value) => Self::UInt64(DecimalU64::new(*value)),
            Value::Locale(value) => Self::Locale((*value).to_owned()),
            Value::Guid(value) => Self::Guid(value.to_string()),
            Value::Single(value) => Self::single(*value),
            Value::Double(value) => Self::double(*value),
            Value::String(value) => Self::Text((*value).to_owned()),
            Value::Enum(value) => Self::Enum((*value).to_owned()),
            Value::Null => return Some(SourceValue::Null),
            Value::Error(_) => return Some(SourceValue::Unreadable),
            _ => return None,
        };
        Some(SourceValue::Present(value))
    }

    pub(crate) fn parse(&self, text: &str) -> Option<Self> {
        match self {
            Self::Boolean(_) if text == "1" || text.eq_ignore_ascii_case("true") => {
                Some(Self::Boolean(true))
            }
            Self::Boolean(_) if text == "0" || text.eq_ignore_ascii_case("false") => {
                Some(Self::Boolean(false))
            }
            Self::Boolean(_) => None,
            Self::Int8(_) => text.parse().ok().map(Self::Int8),
            Self::Int16(_) => text.parse().ok().map(Self::Int16),
            Self::Int32(_) => text.parse().ok().map(Self::Int32),
            Self::Int64(_) => text
                .parse::<i64>()
                .ok()
                .map(|value| Self::Int64(value.to_string())),
            Self::UInt8(_) => text.parse().ok().map(Self::UInt8),
            Self::UInt16(_) => text.parse().ok().map(Self::UInt16),
            Self::UInt32(_) => text.parse().ok().map(Self::UInt32),
            Self::UInt64(_) => text
                .parse()
                .ok()
                .map(|value| Self::UInt64(DecimalU64::new(value))),
            Self::Single(_) => text
                .parse::<f32>()
                .ok()
                .filter(|value| value.is_finite())
                .map(Self::single),
            Self::Double(_) => text
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .map(Self::double),
            Self::Text(_) => Some(Self::Text(text.to_owned())),
            Self::Locale(_) => Some(Self::Locale(text.to_owned())),
            Self::Guid(_) => text
                .parse::<Guid>()
                .ok()
                .map(|value| Self::Guid(value.to_string())),
            Self::Enum(_) => (!text.is_empty()).then(|| Self::Enum(text.to_owned())),
        }
    }

    fn single(value: f32) -> Self {
        Self::Single(Float32 {
            bits: value.to_bits(),
            finite: value.is_finite().then_some(value),
        })
    }

    fn double(value: f64) -> Self {
        Self::Double(Float64 {
            bits: DecimalU64::new(value.to_bits()),
            finite: value.is_finite().then_some(value),
        })
    }
}
