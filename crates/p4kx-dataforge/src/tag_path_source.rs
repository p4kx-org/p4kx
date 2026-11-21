// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Record;

/// The source field whose stored occurrence selected a tag-tree edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TagPathEdgeKind {
    /// A `TagDatabase.tags` root occurrence.
    Root,
    /// A `Tag.children` occurrence.
    Child,
}

impl TagPathEdgeKind {
    /// Returns the original owned array field.
    #[must_use]
    pub const fn field(self) -> &'static str {
        match self {
            Self::Root => "tags",
            Self::Child => "children",
        }
    }
}

/// The original owner and slot of one selected tag-tree edge.
#[derive(Debug, Clone, Copy)]
pub struct TagPathIncoming<'r, 'f> {
    record: &'r Record<'f>,
    kind: TagPathEdgeKind,
    ordinal: usize,
}

impl<'r, 'f> TagPathIncoming<'r, 'f> {
    pub(crate) const fn new(record: &'r Record<'f>, kind: TagPathEdgeKind, ordinal: usize) -> Self {
        Self {
            record,
            kind,
            ordinal,
        }
    }

    /// Borrows the actual source Record that owns the selected edge.
    #[must_use]
    pub const fn record(self) -> &'r Record<'f> {
        self.record
    }

    /// Returns the original array's semantic role.
    #[must_use]
    pub const fn kind(self) -> TagPathEdgeKind {
        self.kind
    }

    /// Returns the original zero-based array position, including preceding null or malformed slots.
    #[must_use]
    pub const fn ordinal(self) -> usize {
        self.ordinal
    }
}
