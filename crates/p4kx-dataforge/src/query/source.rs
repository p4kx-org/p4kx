// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use crate::record::RecordNameKind;
use crate::scalar::ScalarSource;
use crate::{DataForge, Value};

/// Original input coordinates actually used by the Query evaluator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum QuerySource {
    /// Original Record headers examined by the canonical GUID or path index.
    RecordLookup {
        /// Original column backing the index.
        kind: RecordNameKind,
        /// Exclusive end of the physical Record positions beginning at zero.
        end: u64,
    },
    /// A physical target selected by a Reference lookup.
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
    /// The selected Reference target's original type field.
    RecordType {
        /// Original Record table position.
        index: u32,
    },
    /// The original `EnumCount` word consulted before decoding an enum selection.
    EnumCount,
    /// The declared length of the original text table consulted by a scalar read.
    TextLength,
    /// The original class-instance block whose count was consulted.
    InstanceBlockBound {
        /// Requested original Struct index.
        structure: u32,
    },
    /// The original block and Struct size consulted for a Query instance.
    InstanceExtent {
        /// Original Struct table position read by the instance lookup.
        structure: u32,
        /// Whether the lookup also used the block start to reach actual instance bytes.
        resolved: bool,
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
    /// An actual parent word examined by a runtime type test.
    AncestorParent {
        /// Original Struct position containing the parent word.
        index: usize,
        /// Stored parent word, including the no-parent sentinel.
        stored_parent: u32,
    },
    /// An addressed traversal input could not be read as bytes.
    Unavailable(QueryUnavailable),
    /// A runtime type test could not establish the rest of its original ancestry.
    AncestorGap {
        /// Unresolved position, which may not exist in the original table.
        index: usize,
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
    /// A scalar slot consulted but not decoded.
    AttemptedValue,
    /// Readable pointer, link or array header bytes used by a failed traversal.
    AttemptedTraversal(TraversalInput),
    /// Text decoded from a value slot.
    Text,
    /// Terminating original byte of a nonempty admitted textual value.
    TextTerminator,
    /// Consulted original text bytes that did not decode.
    AttemptedText,
    /// One original value-pool cardinality word in the DataForge header.
    PoolBound,
    /// Array count and first-element words.
    Array,
    /// Strong or weak pointer words.
    Pointer,
    /// A Reference used by a follow step.
    Reference,
    /// An original Reference whose non-null target was absent.
    DanglingReference,
    /// A weak pointer used by a follow step.
    WeakLink,
    /// A followed slot that resolved to no target.
    Unresolved,
}

/// The original operand addressed by a query traversal step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum TraversalInput {
    /// A scalar leaf.
    Value,
    /// A strong pointer.
    Pointer,
    /// A Reference link.
    Reference,
    /// A weak pointer link.
    WeakLink,
    /// The authored count/first words of an array.
    Array,
}

/// An attempted traversal coordinate whose requested bytes were unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum QueryUnavailable {
    /// The text offset reached no readable bytes in the original text table.
    #[non_exhaustive]
    Text {
        /// Original offset relative to the text table.
        offset: u32,
    },
    /// A requested slot extends beyond its current original owner.
    #[non_exhaustive]
    Field {
        /// Original decoded start of the available owner bytes, when known.
        owner_start: Option<u64>,
        /// Slot offset relative to that owner.
        offset: u64,
        /// Requested byte count.
        byte_count: u64,
        /// The addressed operand.
        input: TraversalInput,
    },
    /// The logical target instance for resolved words had no original readable bytes.
    #[non_exhaustive]
    Instance {
        /// Original target Struct table position.
        structure: u32,
        /// Requested instance position within its block.
        instance: u32,
    },
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

impl From<ScalarSource> for QuerySource {
    fn from(source: ScalarSource) -> Self {
        match source {
            ScalarSource::EnumCount => Self::EnumCount,
            ScalarSource::TextLength => Self::TextLength,
            ScalarSource::TextTerminator { start, end } => Self::Bytes {
                start,
                end,
                kind: QueryByteUse::TextTerminator,
            },
            ScalarSource::AttemptedText { start, end } => Self::Bytes {
                start,
                end,
                kind: QueryByteUse::AttemptedText,
            },
            ScalarSource::UnavailableText { offset } => {
                Self::Unavailable(QueryUnavailable::Text { offset })
            }
        }
    }
}

pub(super) fn value_sources(
    forge: &DataForge<'_>,
    bytes: Option<&[u8]>,
    value: &Value<'_>,
) -> [Option<QuerySource>; 2] {
    let text = match value {
        Value::String(text) | Value::Locale(text) | Value::Enum(text) => {
            forge.original_text_span(text)
        }
        _ => None,
    };
    [
        bytes.and_then(|bytes| QuerySource::bytes(forge, bytes, QueryByteUse::Value)),
        text.map(|span| QuerySource::Bytes {
            start: span.start,
            end: span.end,
            kind: QueryByteUse::Text,
        }),
    ]
}
