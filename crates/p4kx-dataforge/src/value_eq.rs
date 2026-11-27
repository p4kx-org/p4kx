// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter::Zip;
use std::slice::Iter;

use crate::{Field, StructValue, Value};

enum Frame<'read, 'source> {
    Fields(Zip<Iter<'read, Field<'source>>, Iter<'read, Field<'source>>>),
    Items(Zip<Iter<'read, Value<'source>>, Iter<'read, Value<'source>>>),
}

enum Next<'read, 'source> {
    Values(&'read Value<'source>, &'read Value<'source>),
    Done,
    Different,
}

impl<'read, 'source> Frame<'read, 'source> {
    fn next(&mut self) -> Next<'read, 'source> {
        match self {
            Self::Fields(fields) => match fields.next() {
                Some((left, right)) if left.name == right.name => {
                    Next::Values(&left.value, &right.value)
                }
                Some(_) => Next::Different,
                None => Next::Done,
            },
            Self::Items(items) => items.next().map_or(Next::Done, |(a, b)| Next::Values(a, b)),
        }
    }
}

impl PartialEq for Value<'_> {
    fn eq(&self, other: &Self) -> bool {
        let mut pending = Vec::new();
        enter(self, other, &mut pending) && complete(pending)
    }
}

impl PartialEq for StructValue<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.type_name == other.type_name
            && self.fields.len() == other.fields.len()
            && complete(vec![Frame::Fields(
                self.fields.iter().zip(other.fields.iter()),
            )])
    }
}

fn complete(mut pending: Vec<Frame<'_, '_>>) -> bool {
    while let Some(frame) = pending.last_mut() {
        match frame.next() {
            Next::Values(left, right) => {
                if !enter(left, right, &mut pending) {
                    return false;
                }
            }
            Next::Done => {
                pending.pop();
            }
            Next::Different => return false,
        }
    }
    true
}

fn enter<'read, 'source>(
    left: &'read Value<'source>,
    right: &'read Value<'source>,
    pending: &mut Vec<Frame<'read, 'source>>,
) -> bool {
    match (left, right) {
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Int8(a), Value::Int8(b)) => a == b,
        (Value::Int16(a), Value::Int16(b)) => a == b,
        (Value::Int32(a), Value::Int32(b)) => a == b,
        (Value::Int64(a), Value::Int64(b)) => a == b,
        (Value::UInt8(a), Value::UInt8(b)) => a == b,
        (Value::UInt16(a), Value::UInt16(b)) => a == b,
        (Value::UInt32(a), Value::UInt32(b)) => a == b,
        (Value::UInt64(a), Value::UInt64(b)) => a == b,
        (Value::Single(a), Value::Single(b)) => a.eq(b),
        (Value::Double(a), Value::Double(b)) => a.eq(b),
        (Value::String(a), Value::String(b))
        | (Value::Locale(a), Value::Locale(b))
        | (Value::Enum(a), Value::Enum(b)) => a == b,
        (Value::Guid(a), Value::Guid(b)) => a == b,
        (Value::Reference(a), Value::Reference(b)) => a == b,
        (Value::Error(a), Value::Error(b)) => a == b,
        (Value::Null, Value::Null) => true,
        (Value::Struct(a), Value::Struct(b)) => {
            if a.type_name != b.type_name || a.fields.len() != b.fields.len() {
                return false;
            }
            pending.push(Frame::Fields(a.fields.iter().zip(b.fields.iter())));
            true
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                return false;
            }
            pending.push(Frame::Items(a.iter().zip(b.iter())));
            true
        }
        _ => false,
    }
}
