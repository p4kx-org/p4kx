// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::observation::RecordXmlSite;
use crate::{Field, RecordTree, Value};
use std::slice::Iter;

#[derive(Clone, Copy)]
pub(super) enum Head<'tree, 'source> {
    Record(&'tree RecordTree<'source>, bool),
    Nested {
        type_name: &'source str,
        value: &'tree Value<'source>,
    },
}

#[derive(Clone, Copy)]
pub(super) struct Name<'tree, 'source> {
    pub(super) text: &'source str,
    pub(super) origin: Option<RecordXmlSite<'tree, 'source>>,
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
        name: Name<'tree, 'source>,
        depth: usize,
    },
}

pub(super) struct Fields<'tree, 'source> {
    entries: &'tree [Field<'source>],
    pub(super) base: usize,
    pub(super) name: Name<'tree, 'source>,
    pub(super) depth: usize,
    next: usize,
}

impl<'tree, 'source> Fields<'tree, 'source> {
    pub(super) const fn new(
        entries: &'tree [Field<'source>],
        base: usize,
        name: Name<'tree, 'source>,
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
