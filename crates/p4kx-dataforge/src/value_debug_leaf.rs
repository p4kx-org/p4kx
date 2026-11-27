// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{Guid, Reference, Value, ValueError};
use std::fmt;

/// Bounded leaf formatting with the same enum variant names and standard formatter flags.
pub(super) enum Leaf<'value, 'source> {
    Bool(bool),
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    UInt8(u8),
    UInt16(u16),
    UInt32(u32),
    UInt64(u64),
    Single(f32),
    Double(f64),
    String(&'source str),
    Locale(&'source str),
    Guid(Guid),
    Enum(&'source str),
    Reference(&'value Reference<'source>),
    Null,
    Error(ValueError),
}

impl<'value, 'source> Leaf<'value, 'source> {
    pub(super) fn new(value: &'value Value<'source>) -> Option<Self> {
        Some(match value {
            Value::Bool(value) => Self::Bool(*value),
            Value::Int8(value) => Self::Int8(*value),
            Value::Int16(value) => Self::Int16(*value),
            Value::Int32(value) => Self::Int32(*value),
            Value::Int64(value) => Self::Int64(*value),
            Value::UInt8(value) => Self::UInt8(*value),
            Value::UInt16(value) => Self::UInt16(*value),
            Value::UInt32(value) => Self::UInt32(*value),
            Value::UInt64(value) => Self::UInt64(*value),
            Value::Single(value) => Self::Single(*value),
            Value::Double(value) => Self::Double(*value),
            Value::String(value) => Self::String(value),
            Value::Locale(value) => Self::Locale(value),
            Value::Guid(value) => Self::Guid(*value),
            Value::Enum(value) => Self::Enum(value),
            Value::Reference(value) => Self::Reference(value),
            Value::Null => Self::Null,
            Value::Error(error) => Self::Error(*error),
            Value::Struct(_) | Value::Array(_) => return None,
        })
    }
}

impl fmt::Debug for Leaf<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(value) => formatter.debug_tuple("Bool").field(value).finish(),
            Self::Int8(value) => formatter.debug_tuple("Int8").field(value).finish(),
            Self::Int16(value) => formatter.debug_tuple("Int16").field(value).finish(),
            Self::Int32(value) => formatter.debug_tuple("Int32").field(value).finish(),
            Self::Int64(value) => formatter.debug_tuple("Int64").field(value).finish(),
            Self::UInt8(value) => formatter.debug_tuple("UInt8").field(value).finish(),
            Self::UInt16(value) => formatter.debug_tuple("UInt16").field(value).finish(),
            Self::UInt32(value) => formatter.debug_tuple("UInt32").field(value).finish(),
            Self::UInt64(value) => formatter.debug_tuple("UInt64").field(value).finish(),
            Self::Single(value) => formatter.debug_tuple("Single").field(value).finish(),
            Self::Double(value) => formatter.debug_tuple("Double").field(value).finish(),
            Self::String(value) => formatter.debug_tuple("String").field(value).finish(),
            Self::Locale(value) => formatter.debug_tuple("Locale").field(value).finish(),
            Self::Guid(value) => formatter.debug_tuple("Guid").field(value).finish(),
            Self::Enum(value) => formatter.debug_tuple("Enum").field(value).finish(),
            Self::Reference(value) => formatter.debug_tuple("Reference").field(value).finish(),
            Self::Null => formatter.write_str("Null"),
            Self::Error(value) => formatter.debug_tuple("Error").field(value).finish(),
        }
    }
}
