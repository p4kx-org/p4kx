// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{Field, FieldStorage, Value};
use std::fmt;
use std::iter::Enumerate;
use std::slice::Iter;

enum Frame<'tree, 'source> {
    Fields(Enumerate<Iter<'tree, Field<'source>>>),
    Items(Enumerate<Iter<'tree, Value<'source>>>),
    Value(&'tree Value<'source>),
    EndStruct,
    EndArray,
}

pub(super) enum Event<'tree, 'source> {
    Field {
        ordinal: usize,
        storage: FieldStorage,
        name: &'source str,
    },
    Element(usize),
    BeginStruct(&'source str),
    EndStruct,
    BeginArray(usize),
    EndArray,
    Leaf(&'tree Value<'source>),
}

/// Formats a fresh complete event traversal while borrowing the original tree.
pub(super) struct Events<'tree, 'source> {
    root: Root<'tree, 'source>,
}

enum Root<'tree, 'source> {
    Fields(&'tree [Field<'source>]),
    Items(&'tree [Value<'source>]),
}

struct Walk<'tree, 'source> {
    pending: Vec<Frame<'tree, 'source>>,
}

impl<'tree, 'source> Events<'tree, 'source> {
    pub(super) const fn fields(fields: &'tree [Field<'source>]) -> Self {
        Self {
            root: Root::Fields(fields),
        }
    }
    pub(super) const fn items(items: &'tree [Value<'source>]) -> Self {
        Self {
            root: Root::Items(items),
        }
    }
}

impl fmt::Debug for Events<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let first = match self.root {
            Root::Fields(fields) => Frame::Fields(fields.iter().enumerate()),
            Root::Items(items) => Frame::Items(items.iter().enumerate()),
        };
        formatter
            .debug_list()
            .entries(Walk {
                pending: vec![first],
            })
            .finish()
    }
}

impl<'tree, 'source> Iterator for Walk<'tree, 'source> {
    type Item = Event<'tree, 'source>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(frame) = self.pending.pop() {
            match frame {
                Frame::Fields(mut fields) => {
                    if let Some((ordinal, field)) = fields.next() {
                        self.pending.push(Frame::Fields(fields));
                        self.pending.push(Frame::Value(field.value()));
                        return Some(Event::Field {
                            ordinal,
                            storage: field.storage(),
                            name: field.name(),
                        });
                    }
                }
                Frame::Items(mut items) => {
                    if let Some((ordinal, value)) = items.next() {
                        self.pending.push(Frame::Items(items));
                        self.pending.push(Frame::Value(value));
                        return Some(Event::Element(ordinal));
                    }
                }
                Frame::Value(value) => match value {
                    Value::Struct(structure) => {
                        self.pending.push(Frame::EndStruct);
                        self.pending
                            .push(Frame::Fields(structure.fields().iter().enumerate()));
                        return Some(Event::BeginStruct(structure.type_name()));
                    }
                    Value::Array(items) => {
                        self.pending.push(Frame::EndArray);
                        self.pending.push(Frame::Items(items.iter().enumerate()));
                        return Some(Event::BeginArray(items.len()));
                    }
                    leaf => return Some(Event::Leaf(leaf)),
                },
                Frame::EndStruct => return Some(Event::EndStruct),
                Frame::EndArray => return Some(Event::EndArray),
            }
        }
        None
    }
}

impl fmt::Debug for Event<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Field {
                ordinal,
                storage,
                name,
            } => formatter
                .debug_struct("Field")
                .field("ordinal", ordinal)
                .field("storage", storage)
                .field("name", name)
                .finish(),
            Self::Element(ordinal) => formatter.debug_tuple("Element").field(ordinal).finish(),
            Self::BeginStruct(name) => formatter.debug_tuple("BeginStruct").field(name).finish(),
            Self::EndStruct => formatter.write_str("EndStruct"),
            Self::BeginArray(count) => formatter.debug_tuple("BeginArray").field(count).finish(),
            Self::EndArray => formatter.write_str("EndArray"),
            Self::Leaf(value) => fmt::Debug::fmt(value, formatter),
        }
    }
}
