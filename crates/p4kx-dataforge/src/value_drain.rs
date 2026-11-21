// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::mem;
use std::vec::IntoIter;

use crate::{Field, StructValue, Value};

enum Drain<'source> {
    Fields(IntoIter<Field<'source>>),
    Items(IntoIter<Value<'source>>),
}

impl<'source> Drain<'source> {
    fn next(&mut self) -> Option<Value<'source>> {
        match self {
            Self::Fields(fields) => fields.next().map(|field| field.value),
            Self::Items(items) => items.next(),
        }
    }
}

impl Drop for StructValue<'_> {
    fn drop(&mut self) {
        if self.fields.is_empty() {
            return;
        }
        let fields = mem::take(&mut self.fields);
        let mut pending = vec![Drain::Fields(fields.into_iter())];
        while let Some(frame) = pending.last_mut() {
            let Some(value) = frame.next() else {
                pending.pop();
                continue;
            };
            match value {
                Value::Struct(mut structure) => {
                    let fields = mem::take(&mut structure.fields);
                    if !fields.is_empty() {
                        pending.push(Drain::Fields(fields.into_iter()));
                    }
                }
                Value::Array(items) if !items.is_empty() => {
                    pending.push(Drain::Items(items.into_vec().into_iter()));
                }
                _ => {}
            }
        }
    }
}
