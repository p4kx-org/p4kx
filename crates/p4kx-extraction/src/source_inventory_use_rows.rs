// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::{Borrow, Cow};
use std::collections::btree_map;
use std::fmt::{self, Debug, Formatter};
use std::iter::Peekable;
use std::slice;
use std::sync::{Arc, OnceLock};

use serde::{Serialize, Serializer};

use crate::source_packed_use::{PackedSourceUse, PackedUseTree};
use crate::source_use::SourceUseDescription;

/// Sorted use rows with lazily expanded borrowed views.
#[derive(Clone)]
pub(super) struct FrozenUseRows(Arc<UseRows>);

struct UseRows {
    live: Vec<Arc<SourceUseDescription>>,
    packed: PackedUseRows,
    expanded: OnceLock<Vec<SourceUseDescription>>,
    contiguous: OnceLock<Vec<SourceUseDescription>>,
}

pub(crate) enum PackedUseRows {
    Snapshot(Vec<(Arc<str>, PackedSourceUse)>),
    Sealed(PackedUseTree),
}

impl PackedUseRows {
    fn iter(&self) -> PackedUseIter<'_> {
        match self {
            Self::Snapshot(rows) => PackedUseIter::Snapshot(rows.iter()),
            Self::Sealed(tree) => PackedUseIter::Sealed(tree.iter()),
        }
    }

    fn get(&self, id: &str) -> Option<(&Arc<str>, &PackedSourceUse)> {
        match self {
            Self::Snapshot(rows) => {
                let position = rows
                    .binary_search_by(|(key, _)| key.as_ref().cmp(id))
                    .ok()?;
                rows.get(position).map(|(key, row)| (key, row))
            }
            Self::Sealed(tree) => tree.get_key_value(id),
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::Snapshot(rows) => rows.len(),
            Self::Sealed(tree) => tree.len(),
        }
    }
}

enum PackedUseIter<'a> {
    Snapshot(slice::Iter<'a, (Arc<str>, PackedSourceUse)>),
    Sealed(btree_map::Iter<'a, Arc<str>, PackedSourceUse>),
}

impl<'a> Iterator for PackedUseIter<'a> {
    type Item = (&'a Arc<str>, &'a PackedSourceUse);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Snapshot(rows) => rows.next().map(|(id, row)| (id, row)),
            Self::Sealed(tree) => tree.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let length = self.len();
        (length, Some(length))
    }
}

impl ExactSizeIterator for PackedUseIter<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Snapshot(rows) => rows.len(),
            Self::Sealed(tree) => tree.len(),
        }
    }
}

impl FrozenUseRows {
    pub(super) fn new(live: Vec<Arc<SourceUseDescription>>, packed: PackedUseRows) -> Self {
        Self(Arc::new(UseRows {
            live,
            packed,
            expanded: OnceLock::new(),
            contiguous: OnceLock::new(),
        }))
    }

    pub(super) fn iter(&self) -> impl ExactSizeIterator<Item = &SourceUseDescription> {
        let expanded = self.0.expanded.get_or_init(|| {
            self.0
                .packed
                .iter()
                .map(|(id, row)| row.restore(id))
                .collect()
        });
        merge(self.0.live.iter().map(Arc::as_ref), expanded.iter())
    }

    pub(super) fn get(&self, id: &str) -> Option<Cow<'_, SourceUseDescription>> {
        if let Ok(position) = self
            .0
            .live
            .binary_search_by(|row| row.id().as_str().cmp(id))
        {
            return self
                .0
                .live
                .get(position)
                .map(|row| Cow::Borrowed(row.as_ref()));
        }
        let (original, row) = self.0.packed.get(id)?;
        Some(Cow::Owned(row.restore(original)))
    }

    pub(super) fn len(&self) -> usize {
        self.0.live.len() + self.0.packed.len()
    }

    pub(super) fn as_slice(&self) -> &[SourceUseDescription] {
        self.0
            .contiguous
            .get_or_init(|| self.views().map(Cow::into_owned).collect())
    }

    fn views(&self) -> impl ExactSizeIterator<Item = Cow<'_, SourceUseDescription>> {
        let live = self.0.live.iter().map(|row| Cow::Borrowed(row.as_ref()));
        let packed = self
            .0
            .packed
            .iter()
            .map(|(id, row)| Cow::Owned(row.restore(id)));
        merge(live, packed)
    }
}

impl Debug for FrozenUseRows {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.views()).finish()
    }
}

impl PartialEq for FrozenUseRows {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.views().eq(other.views())
    }
}

impl Eq for FrozenUseRows {}

impl Serialize for FrozenUseRows {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.views())
    }
}

fn merge<Row, Left, Right>(left: Left, right: Right) -> impl ExactSizeIterator<Item = Row>
where
    Row: Borrow<SourceUseDescription>,
    Left: ExactSizeIterator<Item = Row>,
    Right: ExactSizeIterator<Item = Row>,
{
    OrderedUseRows {
        left: left.peekable(),
        right: right.peekable(),
    }
}

struct OrderedUseRows<Left: Iterator, Right: Iterator> {
    left: Peekable<Left>,
    right: Peekable<Right>,
}

impl<Row, Left, Right> Iterator for OrderedUseRows<Left, Right>
where
    Row: Borrow<SourceUseDescription>,
    Left: ExactSizeIterator<Item = Row>,
    Right: ExactSizeIterator<Item = Row>,
{
    type Item = Row;

    fn next(&mut self) -> Option<Self::Item> {
        match (self.left.peek(), self.right.peek()) {
            (Some(left), Some(right)) if left.borrow().id() <= right.borrow().id() => {
                self.left.next()
            }
            (_, Some(_)) => self.right.next(),
            (_, None) => self.left.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let length = self.len();
        (length, Some(length))
    }
}

impl<Row, Left, Right> ExactSizeIterator for OrderedUseRows<Left, Right>
where
    Row: Borrow<SourceUseDescription>,
    Left: ExactSizeIterator<Item = Row>,
    Right: ExactSizeIterator<Item = Row>,
{
    fn len(&self) -> usize {
        self.left.len() + self.right.len()
    }
}

#[cfg(test)]
#[path = "source_inventory_use_lookup_tests.rs"]
mod lookup_tests;
