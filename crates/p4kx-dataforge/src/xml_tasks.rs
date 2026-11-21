// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{Field, RecordTree, Value};
use std::slice::Iter;

#[derive(Clone, Copy)]
pub(super) enum Head<'tree, 'source> {
    Record(&'tree RecordTree<'source>, bool),
    Nested(&'source str),
}

pub(super) enum Task<'tree, 'source> {
    Structure {
        head: Head<'tree, 'source>,
        fields: &'tree [Field<'source>],
        depth: usize,
    },
    Fields(Fields<'tree, 'source>),
    Child(&'tree Field<'source>, usize),
    Value(&'tree Value<'source>, usize),
    Items(Iter<'tree, Value<'source>>, usize),
    CloseField {
        name: &'source str,
        depth: usize,
    },
}

pub(super) struct Fields<'tree, 'source> {
    entries: &'tree [Field<'source>],
    pub(super) base: usize,
    pub(super) name: &'source str,
    pub(super) depth: usize,
    next: usize,
}

impl<'tree, 'source> Fields<'tree, 'source> {
    pub(super) const fn new(
        entries: &'tree [Field<'source>],
        base: usize,
        name: &'source str,
        depth: usize,
    ) -> Self {
        Self {
            entries,
            base,
            name,
            depth,
            next: 0,
        }
    }

    pub(super) fn next(&mut self, flags: &[bool]) -> Option<&'tree Field<'source>> {
        while let Some(field) = self.entries.get(self.next) {
            let index = self.next;
            self.next += 1;
            if flags.get(self.base + index) == Some(&false) {
                return Some(field);
            }
        }
        None
    }
}
