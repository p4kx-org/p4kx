// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{StructValue, Value};

/// Returns the struct a value holds, or `None` for anything else.
pub(crate) const fn as_struct<'v, 'f>(value: &'v Value<'f>) -> Option<&'v StructValue<'f>> {
    match value {
        Value::Struct(structure) => Some(structure),
        _ => None,
    }
}

/// Returns the struct in the field `name`, or `None` when it holds none.
pub(crate) fn struct_field<'v, 'f>(
    structure: &'v StructValue<'f>,
    name: &str,
) -> Option<&'v StructValue<'f>> {
    structure.field(name).and_then(as_struct)
}

/// Returns the structs in the array field `name`, skipping elements that hold none.
pub(crate) fn struct_items<'v, 'f>(
    structure: &'v StructValue<'f>,
    name: &str,
) -> impl Iterator<Item = &'v StructValue<'f>> {
    let items = match structure.field(name) {
        Some(Value::Array(items)) => &items[..],
        _ => &[],
    };
    items.iter().filter_map(as_struct)
}

/// Returns whether the array field `name` holds at least one element.
pub(crate) fn has_items(structure: &StructValue<'_>, name: &str) -> bool {
    matches!(structure.field(name), Some(Value::Array(items)) if !items.is_empty())
}

/// Returns the text of a string, localisation key or enum field, or an empty string.
pub(crate) fn text<'f>(structure: &StructValue<'f>, name: &str) -> &'f str {
    structure.field(name).map_or("", value_text)
}

/// Returns the text of a string, localisation key or enum value, or an empty string.
pub(crate) const fn value_text<'f>(value: &Value<'f>) -> &'f str {
    match value {
        Value::String(text) | Value::Locale(text) | Value::Enum(text) => text,
        _ => "",
    }
}

/// Returns the texts of an array field of strings, localisation keys or enum values.
pub(crate) fn texts(structure: &StructValue<'_>, name: &str) -> Vec<String> {
    match structure.field(name) {
        Some(Value::Array(items)) => items
            .iter()
            .map(value_text)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

/// Returns whether the boolean field `name` is set; a missing or non-boolean field is not.
pub(crate) fn flag(structure: &StructValue<'_>, name: &str) -> bool {
    matches!(structure.field(name), Some(Value::Bool(true)))
}

/// Returns an integer field as `u32`, or `None` when it holds no integer in range.
pub(crate) fn unsigned(structure: &StructValue<'_>, name: &str) -> Option<u32> {
    unsigned_value(structure.field(name)?)
}

/// Returns the integers of an array field that are in range as `u32`.
pub(crate) fn unsigned_list(structure: &StructValue<'_>, name: &str) -> Vec<u32> {
    match structure.field(name) {
        Some(Value::Array(items)) => items.iter().filter_map(unsigned_value).collect(),
        _ => Vec::new(),
    }
}

fn unsigned_value(value: &Value<'_>) -> Option<u32> {
    match value {
        Value::UInt8(value) => Some(u32::from(*value)),
        Value::UInt16(value) => Some(u32::from(*value)),
        Value::UInt32(value) => Some(*value),
        Value::UInt64(value) => u32::try_from(*value).ok(),
        Value::Int8(value) => u32::try_from(*value).ok(),
        Value::Int16(value) => u32::try_from(*value).ok(),
        Value::Int32(value) => u32::try_from(*value).ok(),
        Value::Int64(value) => u32::try_from(*value).ok(),
        _ => None,
    }
}

/// Returns a numeric field as `f64`, or `None` when it holds none.
pub(crate) fn float(structure: &StructValue<'_>, name: &str) -> Option<f64> {
    match structure.field(name)? {
        Value::Single(value) => Some(f64::from(*value)),
        Value::Double(value) => Some(*value),
        _ => None,
    }
}

/// Returns the fields `x`, `y` and `z` of a struct as floats, zero where one is missing.
pub(crate) fn xyz(structure: Option<&StructValue<'_>>) -> [f32; 3] {
    ["x", "y", "z"].map(|name| match structure.and_then(|s| s.field(name)) {
        Some(Value::Single(value)) => *value,
        Some(Value::Double(value)) => narrow(*value),
        _ => 0.0,
    })
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "a double coordinate is given at single precision"
)]
pub(crate) const fn narrow(value: f64) -> f32 {
    value as f32
}
