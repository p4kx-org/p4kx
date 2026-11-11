// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter::Zip;
use std::slice::{Iter, IterMut};

use crate::{Field, StructValue, Value};

enum Frame<'read, 'source> {
    Fields(Zip<Iter<'read, Field<'source>>, IterMut<'read, Field<'source>>>),
    Items(Zip<Iter<'read, Value<'source>>, IterMut<'read, Value<'source>>>),
}

impl<'read, 'source> Frame<'read, 'source> {
    fn next(&mut self) -> Option<(&'read Value<'source>, &'read mut Value<'source>)> {
        match self {
            Self::Fields(fields) => fields.next().map(|(a, b)| (&a.value, &mut b.value)),
            Self::Items(items) => items.next(),
        }
    }
}

impl Clone for Value<'_> {
    fn clone(&self) -> Self {
        let mut target = shallow(self);
        complete(self, &mut target);
        target
    }
}

impl Clone for StructValue<'_> {
    fn clone(&self) -> Self {
        let mut target = Self {
            type_name: self.type_name,
            struct_index: self.struct_index,
            fields: shallow_fields(&self.fields),
        };
        complete_frames(vec![Frame::Fields(
            self.fields.iter().zip(target.fields.iter_mut()),
        )]);
        target
    }
}

fn shallow_fields<'source>(fields: &[Field<'source>]) -> Vec<Field<'source>> {
    fields
        .iter()
        .map(|field| Field {
            storage: field.storage,
            name: field.name,
            value: shallow(&field.value),
        })
        .collect()
}

fn complete<'source>(source: &Value<'source>, target: &mut Value<'source>) {
    let mut pending = Vec::new();
    enter(source, target, &mut pending);
    complete_frames(pending);
}

fn complete_frames(mut pending: Vec<Frame<'_, '_>>) {
    while let Some(frame) = pending.last_mut() {
        if let Some((source, target)) = frame.next() {
            enter(source, target, &mut pending);
        } else {
            pending.pop();
        }
    }
}

fn enter<'read, 'source>(
    source: &'read Value<'source>,
    target: &'read mut Value<'source>,
    pending: &mut Vec<Frame<'read, 'source>>,
) {
    match (source, target) {
        (Value::Struct(source), Value::Struct(target)) => {
            target.fields = shallow_fields(&source.fields);
            pending.push(Frame::Fields(
                source.fields.iter().zip(target.fields.iter_mut()),
            ));
        }
        (Value::Array(source), Value::Array(target)) => {
            *target = source.iter().map(shallow).collect();
            pending.push(Frame::Items(source.iter().zip(target.iter_mut())));
        }
        _ => {}
    }
}

fn shallow<'source>(value: &Value<'source>) -> Value<'source> {
    match value {
        Value::Bool(v) => Value::Bool(*v),
        Value::Int8(v) => Value::Int8(*v),
        Value::Int16(v) => Value::Int16(*v),
        Value::Int32(v) => Value::Int32(*v),
        Value::Int64(v) => Value::Int64(*v),
        Value::UInt8(v) => Value::UInt8(*v),
        Value::UInt16(v) => Value::UInt16(*v),
        Value::UInt32(v) => Value::UInt32(*v),
        Value::UInt64(v) => Value::UInt64(*v),
        Value::Single(v) => Value::Single(*v),
        Value::Double(v) => Value::Double(*v),
        Value::String(v) => Value::String(v),
        Value::Locale(v) => Value::Locale(v),
        Value::Guid(v) => Value::Guid(*v),
        Value::Enum(v) => Value::Enum(v),
        Value::Struct(v) => Value::Struct(StructValue {
            type_name: v.type_name,
            struct_index: v.struct_index,
            fields: Vec::new(),
        }),
        Value::Array(_) => Value::Array(Box::default()),
        Value::Reference(v) => Value::Reference(v.clone()),
        Value::Null => Value::Null,
        Value::Error(v) => Value::Error(*v),
    }
}
