// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use crate::record::RecordNameKind;
use crate::{DataForge, Value};

/// Original input coordinates actually used by the Query evaluator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum QuerySource {
    /// A physical target of the Query's Reference-index lookup.
    ReferenceTarget {
        /// Original target Record table index.
        index: u32,
        /// Original column consulted by the target lookup.
        kind: RecordNameKind,
    },
    /// An actual incoming Reference occurrence consulted by the Query.
    ReferenceEdge {
        /// Original Record that holds this occurrence.
        source: u32,
        /// Original canonical value position excluding the root.
        ordinal: u64,
        /// Original Record admitted as the target.
        target: u32,
    },
    /// Original Records actually traversed by the observed Reference scan.
    ReferenceScan {
        /// Exclusive end of the Record range beginning at zero.
        end: u64,
    },
    /// An unread original position that may hide a Reference.
    ReferenceGap {
        /// Original Record table index.
        record: u32,
        /// Original canonical value position, absent for an unreadable Record.
        ordinal: Option<u64>,
    },
    /// Half-open range in the decoded DCB.
    Bytes {
        /// Inclusive offset.
        start: u64,
        /// Exclusive offset.
        end: u64,
        /// The decoder's use of these bytes.
        kind: QueryByteUse,
    },
    /// A Record selected by a Reference.
    Record {
        /// Original Record table index.
        index: u32,
    },
    /// One selected element of an original instance array.
    ArrayElement {
        /// Original field header start in the decoded DCB.
        header_start: u64,
        /// Zero-based position inside the original array.
        index: u32,
    },
    /// The physical instance admitted by a pointer or array.
    Instance {
        /// Original Struct index.
        structure: u32,
        /// Position in that Struct's instance block.
        instance: u32,
    },
    /// An examined Struct definition.
    Structure {
        /// Original Struct table index.
        index: u32,
    },
}

impl QuerySource {
    pub(super) fn bytes(forge: &DataForge<'_>, bytes: &[u8], kind: QueryByteUse) -> Option<Self> {
        let span = forge.byte_span(bytes)?;
        Some(Self::Bytes {
            start: span.start,
            end: span.end,
            kind,
        })
    }
}

/// The role of one observed byte range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum QueryByteUse {
    /// A scalar value slot.
    Value,
    /// Text decoded from a value slot.
    Text,
    /// Array count and first-element words.
    Array,
    /// Strong or weak pointer words.
    Pointer,
    /// A Reference used by a follow step.
    Reference,
    /// A weak pointer used by a follow step.
    WeakLink,
    /// A followed slot that resolved to no target.
    Unresolved,
}

/// Evidence for the emitted values and for the traversal that determined the shape of a cell.
#[derive(Debug, Clone, Default)]
pub struct QueryCellSources {
    traversed: BTreeSet<QuerySource>,
    follow_inputs: BTreeSet<QuerySource>,
    values: Vec<Box<[QuerySource]>>,
}

impl QueryCellSources {
    /// Returns the distinct original inputs actually examined for this cell.
    pub fn traversed(&self) -> impl Iterator<Item = QuerySource> + '_ {
        self.traversed.iter().copied()
    }

    /// Returns the original selectors consulted to reach and resolve links.
    pub fn follow_inputs(&self) -> impl Iterator<Item = QuerySource> + '_ {
        self.follow_inputs.iter().copied()
    }

    /// Returns the selected route and payload inputs for each value in output order.
    #[must_use]
    pub fn values(&self) -> &[Box<[QuerySource]>] {
        &self.values
    }
}

#[derive(Debug, Default)]
pub(super) struct CellTrace {
    evidence: QueryCellSources,
    path: Vec<QuerySource>,
}

impl CellTrace {
    pub(super) const fn depth(&self) -> usize {
        self.path.len()
    }

    pub(super) fn restore(&mut self, depth: usize) {
        self.path.truncate(depth);
    }

    pub(super) fn route(&mut self, source: QuerySource) {
        self.evidence.traversed.insert(source);
        self.path.push(source);
    }

    pub(super) fn observe_follow_route(&mut self) {
        self.evidence
            .follow_inputs
            .extend(self.path.iter().copied());
    }

    pub(super) fn unresolved(&mut self, source: QuerySource) {
        self.evidence.traversed.insert(source);
        self.evidence.follow_inputs.insert(source);
    }

    pub(super) fn value(&mut self, forge: &DataForge<'_>, bytes: Option<&[u8]>, value: &Value<'_>) {
        let payload = value_sources(forge, bytes, value);
        let mut sources = self.path.clone();
        sources.extend(payload.into_iter().flatten());
        self.evidence.traversed.extend(sources.iter().copied());
        self.evidence.values.push(sources.into_boxed_slice());
    }

    pub(super) fn finish(self) -> QueryCellSources {
        self.evidence
    }
}

pub(super) fn value_sources(
    forge: &DataForge<'_>,
    bytes: Option<&[u8]>,
    value: &Value<'_>,
) -> [Option<QuerySource>; 2] {
    let text = match value {
        Value::String(text) | Value::Locale(text) | Value::Enum(text) => Some(text.as_bytes()),
        _ => None,
    };
    [
        bytes.and_then(|bytes| QuerySource::bytes(forge, bytes, QueryByteUse::Value)),
        text.and_then(|bytes| QuerySource::bytes(forge, bytes, QueryByteUse::Text)),
    ]
}
