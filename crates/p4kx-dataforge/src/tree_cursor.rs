// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{DataType, Field, Step, Value};
use std::iter::Zip;
use std::ops::Range;
use std::slice::Iter;

enum Entries<'tree, 'source> {
    Fields(Iter<'tree, Field<'source>>),
    Items {
        items: Zip<Range<u32>, Iter<'tree, Value<'source>>>,
        field: &'source str,
        data_type: DataType,
    },
}

struct Frame<'tree, 'source> {
    entries: Entries<'tree, 'source>,
    prefix: usize,
}

struct Item<'tree, 'source> {
    value: &'tree Value<'source>,
    step: Step<'source>,
    field: &'source str,
    data_type: DataType,
}

impl<'tree, 'source> Frame<'tree, 'source> {
    fn next(&mut self) -> Option<Item<'tree, 'source>> {
        match &mut self.entries {
            Entries::Fields(fields) => {
                let field = fields.next()?;
                Some(Item {
                    value: field.value(),
                    step: Step::Field(field.name()),
                    field: field.name(),
                    data_type: field.storage().data_type(),
                })
            }
            Entries::Items {
                items,
                field,
                data_type,
            } => {
                let (index, value) = items.next()?;
                let field = *field;
                Some(Item {
                    value,
                    step: Step::Element(index),
                    field,
                    data_type: *data_type,
                })
            }
        }
    }
}

/// Borrows one original-order occurrence and the cursor's current semantic path.
pub(crate) struct Visit<'visit, 'tree, 'source> {
    pub(crate) value: &'tree Value<'source>,
    pub(crate) field: &'source str,
    pub(crate) steps: &'visit [Step<'source>],
    pub(crate) data_type: DataType,
}

/// Traverses canonical owned values without recursively borrowing child iterators.
pub(crate) struct Cursor<'tree, 'source> {
    pending: Vec<Frame<'tree, 'source>>,
    steps: Vec<Step<'source>>,
}

impl<'tree, 'source> Cursor<'tree, 'source> {
    pub(crate) fn new(fields: &'tree [Field<'source>]) -> Self {
        Self {
            pending: vec![Frame {
                entries: Entries::Fields(fields.iter()),
                prefix: 0,
            }],
            steps: Vec::new(),
        }
    }

    pub(crate) fn next(&mut self) -> Option<Visit<'_, 'tree, 'source>> {
        while let Some(frame) = self.pending.last_mut() {
            self.steps.truncate(frame.prefix);
            let Some(Item {
                value,
                step,
                field,
                data_type,
            }) = frame.next()
            else {
                self.pending.pop();
                continue;
            };
            self.steps.push(step);
            let entries = match value {
                Value::Struct(structure) => Some(Entries::Fields(structure.fields().iter())),
                Value::Array(items) => Some(Entries::Items {
                    items: (0..u32::MAX).zip(items.iter()),
                    field,
                    data_type,
                }),
                _ => None,
            };
            if let Some(entries) = entries {
                self.pending.push(Frame {
                    entries,
                    prefix: self.steps.len(),
                });
            }
            return Some(Visit {
                value,
                field,
                steps: &self.steps,
                data_type,
            });
        }
        None
    }
}
