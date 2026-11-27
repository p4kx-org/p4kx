// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter::Enumerate;
use std::slice::Iter;

use super::observation::RecordXmlSite;
use crate::{Field, PositionedFields, PositionedValue, RecordTree, Value};

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
    pub(super) position: Option<PositionedValue<'tree, 'source>>,
}

pub(super) enum Task<'tree, 'source> {
    Structure(Structure<'tree, 'source>),
    Fields(Fields<'tree, 'source>),
    Child(
        &'tree Field<'source>,
        Option<PositionedValue<'tree, 'source>>,
        usize,
    ),
    Value(
        &'tree Value<'source>,
        Option<PositionedValue<'tree, 'source>>,
        usize,
    ),
    Items(Items<'tree, 'source>, usize),
    LeaveContainer,
    CloseField {
        name: Name<'tree, 'source>,
        depth: usize,
    },
}

pub(super) struct Structure<'tree, 'source> {
    pub(super) head: Head<'tree, 'source>,
    pub(super) fields: &'tree [Field<'source>],
    pub(super) positions: Option<PositionedFields<'tree, 'source>>,
    pub(super) container: Option<PositionedValue<'tree, 'source>>,
    pub(super) depth: usize,
}

pub(super) struct Fields<'tree, 'source> {
    entries: &'tree [Field<'source>],
    positions: Option<PositionedFields<'tree, 'source>>,
    pub(super) base: usize,
    pub(super) name: Name<'tree, 'source>,
    pub(super) depth: usize,
    next: usize,
}

impl<'tree, 'source> Fields<'tree, 'source> {
    pub(super) const fn new(
        entries: &'tree [Field<'source>],
        positions: Option<PositionedFields<'tree, 'source>>,
        base: usize,
        name: Name<'tree, 'source>,
        depth: usize,
    ) -> Self {
        Self {
            entries,
            positions,
            base,
            name,
            depth,
            next: 0,
        }
    }

    pub(super) fn next(
        &mut self,
        flags: &[bool],
    ) -> Option<(
        &'tree Field<'source>,
        Option<PositionedValue<'tree, 'source>>,
    )> {
        while let Some(field) = self.entries.get(self.next) {
            let index = self.next;
            self.next += 1;
            if flags.get(self.base + index) == Some(&false) {
                let position = self
                    .positions
                    .as_ref()
                    .and_then(|positions| positions.get(index));
                return Some((field, position));
            }
        }
        None
    }
}

pub(super) struct Items<'tree, 'source> {
    values: Enumerate<Iter<'tree, Value<'source>>>,
    source: Option<PositionedValue<'tree, 'source>>,
}

impl<'tree, 'source> Items<'tree, 'source> {
    pub(super) fn new(
        values: &'tree [Value<'source>],
        source: Option<PositionedValue<'tree, 'source>>,
    ) -> Self {
        Self {
            values: values.iter().enumerate(),
            source,
        }
    }

    pub(super) fn next(
        &mut self,
    ) -> Option<(
        &'tree Value<'source>,
        Option<PositionedValue<'tree, 'source>>,
    )> {
        let (ordinal, value) = self.values.next()?;
        let position = self.source.and_then(|source| {
            if matches!(source.value(), Value::Array(_)) {
                source.element(u32::try_from(ordinal).ok()?)
            } else {
                Some(source)
            }
        });
        Some((value, position))
    }
}
