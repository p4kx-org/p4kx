// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::{Borrow, Cow};
use std::fmt::{self, Debug, Formatter};
use std::iter::Peekable;
use std::sync::{Arc, OnceLock};

use serde::{Serialize, Serializer};

use crate::source_packed_use::PackedSourceUse;
use crate::source_use::{SourceUseDescription, SourceUseRef};

/// Sorted use rows with lazily expanded borrowed views.
#[derive(Clone)]
pub(super) struct FrozenUseRows(Arc<UseRows>);

struct UseRows {
    live: Vec<Arc<SourceUseDescription>>,
    packed: Vec<(SourceUseRef, PackedSourceUse)>,
    expanded: OnceLock<Vec<SourceUseDescription>>,
    contiguous: OnceLock<Vec<SourceUseDescription>>,
}

impl FrozenUseRows {
    pub(super) fn new(
        live: Vec<Arc<SourceUseDescription>>,
        packed: Vec<(SourceUseRef, PackedSourceUse)>,
    ) -> Self {
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
