// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{Field, RecordTree, Value};
use std::slice::Iter;

pub(super) enum Task<'tree, 'source> {
    Structure {
        record: Option<&'tree RecordTree<'source>>,
        type_name: &'source str,
        fields: &'tree [Field<'source>],
        depth: usize,
    },
    Fields(Fields<'tree, 'source>),
    Value(&'tree Value<'source>, usize),
    Items(Items<'tree, 'source>),
    CloseWrapper(usize),
}

enum Phase {
    Attributes,
    Children,
}

pub(super) struct Fields<'tree, 'source> {
    entries: &'tree [Field<'source>],
    pub(super) attributes: usize,
    pub(super) escaped: usize,
    pub(super) depth: usize,
    next: usize,
    phase: Phase,
}

#[derive(Clone, Copy)]
pub(super) struct Selected<'tree, 'source> {
    pub(super) index: usize,
    pub(super) field: &'tree Field<'source>,
    pub(super) escaped: bool,
}

impl<'tree, 'source> Fields<'tree, 'source> {
    pub(super) const fn new(
        entries: &'tree [Field<'source>],
        attributes: usize,
        escaped: usize,
        depth: usize,
    ) -> Self {
        Self {
            entries,
            attributes,
            escaped,
            depth,
            next: 0,
            phase: Phase::Attributes,
        }
    }

    pub(super) fn next(
        &mut self,
        flags: &[bool],
        escaped: &[bool],
    ) -> Option<Selected<'tree, 'source>> {
        loop {
            while let Some(field) = self.entries.get(self.next) {
                let index = self.next;
                self.next += 1;
                let attributes = matches!(self.phase, Phase::Attributes);
                if flags.get(self.attributes + index) == Some(&attributes) {
                    return Some(Selected {
                        index,
                        field,
                        escaped: escaped.get(self.escaped + index) == Some(&true),
                    });
                }
            }
            match self.phase {
                Phase::Attributes => {
                    self.phase = Phase::Children;
                    self.next = 0;
                }
                Phase::Children => return None,
            }
        }
    }
}

pub(super) struct Items<'tree, 'source> {
    pending: Vec<Iter<'tree, Value<'source>>>,
    pub(super) depth: usize,
    pub(super) first: bool,
}

impl<'tree, 'source> Items<'tree, 'source> {
    pub(super) fn new(items: &'tree [Value<'source>], depth: usize) -> Self {
        Self {
            pending: vec![items.iter()],
            depth,
            first: true,
        }
    }

    pub(super) fn next(&mut self) -> Option<&'tree Value<'source>> {
        while let Some(items) = self.pending.last_mut() {
            match items.next() {
                Some(Value::Array(inner)) => self.pending.push(inner.iter()),
                Some(item) => return Some(item),
                None => {
                    self.pending.pop();
                }
            }
        }
        None
    }
}
