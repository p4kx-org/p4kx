// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{Reference, StructValue, Value};

#[expect(
    clippy::cast_possible_truncation,
    reason = "layout values are single-precision floats"
)]
#[expect(
    clippy::cast_precision_loss,
    reason = "integer layout values are far below 2^24"
)]
pub(crate) fn float(structure: &StructValue<'_>, name: &str) -> Option<f32> {
    match structure.field(name)? {
        Value::Single(value) => Some(*value),
        Value::Double(value) => Some(*value as f32),
        Value::Int32(value) => Some(*value as f32),
        Value::UInt32(value) => Some(*value as f32),
        Value::Int64(value) => Some(*value as f32),
        Value::UInt64(value) => Some(*value as f32),
        Value::Int16(value) => Some(f32::from(*value)),
        Value::UInt16(value) => Some(f32::from(*value)),
        Value::Int8(value) => Some(f32::from(*value)),
        Value::UInt8(value) => Some(f32::from(*value)),
        _ => None,
    }
}

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
