// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::header::NODE_LEN;
use crate::{ByteOrder, XmlFieldKind};

/// The source fact in the original decoded CryXml bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum XmlRawKind {
    /// The node's name offset word.
    NodeNameOffset,
    /// The node's content offset word.
    NodeTextOffset,
    /// The attribute's name offset word.
    AttributeNameOffset,
    /// The attribute's value offset word.
    AttributeValueOffset,
    /// The node's original attribute count.
    AttributeCount,
    /// The node's first attribute index.
    FirstAttribute,
    /// The node's parent used to build the tree.
    NodeParent,
    /// The actual string-table bytes read for this output span.
    StringBytes,
    /// A string prefix skipped to reach a UTF-8 character boundary.
    StringBoundary,
    /// The NUL ending the selected string.
    StringTerminator,
}

impl XmlRawKind {
    /// Returns the stable name of the original fact.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NodeNameOffset => "nodeNameOffset",
            Self::NodeTextOffset => "nodeTextOffset",
            Self::AttributeNameOffset => "attributeNameOffset",
            Self::AttributeValueOffset => "attributeValueOffset",
            Self::AttributeCount => "attributeCount",
            Self::FirstAttribute => "firstAttribute",
            Self::NodeParent => "nodeParent",
            Self::StringBytes => "stringBytes",
            Self::StringBoundary => "stringBoundary",
            Self::StringTerminator => "stringTerminator",
        }
    }

    /// Returns whether this fact is used to select or delimit the copied input.
    #[must_use]
    pub const fn is_selector(self) -> bool {
        !matches!(self, Self::StringBytes)
    }
}

/// An original half-open range in the decoded CryXml input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlRawSpan {
    kind: XmlRawKind,
    range: Range<u64>,
}

impl XmlRawSpan {
    pub(crate) const fn new(kind: XmlRawKind, range: Range<u64>) -> Self {
        Self { kind, range }
    }

    /// Returns the decoded-input byte range.
    #[must_use]
    pub const fn range(&self) -> &Range<u64> {
        &self.range
    }

    /// Returns the original fact represented by the bytes.
    #[must_use]
    pub const fn kind(&self) -> XmlRawKind {
        self.kind
    }
}

/// The content produced in an XML output span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum XmlRenderKind {
    /// The renderer's fixed XML declaration.
    Declaration,
    /// One field or one line section of an element's text.
    Field(XmlFieldKind),
    /// Punctuation, whitespace or a structural formatting decision.
    Structure,
}

/// Original later parent words used to exclude children of a CryXml node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlParentWords {
    first: u64,
    count: u32,
}

impl XmlParentWords {
    pub(crate) const fn new(first: u64, count: u32) -> Self {
        Self { first, count }
    }

    /// Returns the first parent-word offset in the decoded CryXml input.
    #[must_use]
    pub const fn first_offset(&self) -> u64 {
        self.first
    }

    /// Returns the number of consecutive node rows whose parent words were surveyed.
    #[must_use]
    pub const fn word_count(&self) -> u32 {
        self.count
    }

    /// Returns the distance in bytes between parent words.
    #[must_use]
    pub const fn stride(&self) -> u64 {
        NODE_LEN as u64
    }

    /// Returns the width in bytes of each original parent word.
    #[must_use]
    pub const fn word_width(&self) -> u64 {
        size_of::<u32>() as u64
    }
}

/// Original node-table inputs proving a CryXml node has no children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlChildAbsence {
    node: u32,
    order: ByteOrder,
    directory: Range<u64>,
    parents: Option<XmlParentWords>,
}

impl XmlChildAbsence {
    pub(crate) const fn new(
        node: u32,
        order: ByteOrder,
        directory: Range<u64>,
        parents: Option<XmlParentWords>,
    ) -> Self {
        Self {
            node,
            order,
            directory,
            parents,
        }
    }

    /// Returns the node whose children were ruled out.
    #[must_use]
    pub const fn node_index(&self) -> u32 {
        self.node
    }

    /// Returns the byte order used to read the directory and parent words.
    #[must_use]
    pub const fn byte_order(&self) -> ByteOrder {
        self.order
    }

    /// Returns the original file size, node-table offset and node-count words.
    #[must_use]
    pub const fn node_directory(&self) -> &Range<u64> {
        &self.directory
    }

    /// Returns the later parent words, or none when the node is last in the table.
    #[must_use]
    pub const fn parent_words(&self) -> Option<&XmlParentWords> {
        self.parents.as_ref()
    }
}

pub(crate) type RawSpans = [Option<XmlRawSpan>; 4];

#[derive(Debug, Clone, PartialEq, Eq)]
enum Contributors {
    Fields(RawSpans),
    Indentation(Vec<XmlRawSpan>),
    ChildAbsence {
        fields: RawSpans,
        absence: XmlChildAbsence,
    },
}

/// Emitted bytes and their original decoded-input ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlRenderSpan {
    kind: XmlRenderKind,
    range: Range<u64>,
    sources: Contributors,
    complete: bool,
}

impl XmlRenderSpan {
    pub(crate) const fn new(
        kind: XmlRenderKind,
        range: Range<u64>,
        sources: RawSpans,
        complete: bool,
    ) -> Self {
        Self {
            kind,
            range,
            sources: Contributors::Fields(sources),
            complete,
        }
    }

    pub(crate) const fn indentation(
        range: Range<u64>,
        sources: Vec<XmlRawSpan>,
        complete: bool,
    ) -> Self {
        Self {
            kind: XmlRenderKind::Structure,
            range,
            sources: Contributors::Indentation(sources),
            complete,
        }
    }

    pub(crate) const fn without_children(
        range: Range<u64>,
        fields: RawSpans,
        absence: XmlChildAbsence,
    ) -> Self {
        Self {
            kind: XmlRenderKind::Structure,
            range,
            sources: Contributors::ChildAbsence { fields, absence },
            complete: false,
        }
    }

    /// Returns the emitted XML byte range or the insertion point for an empty value.
    #[must_use]
    pub const fn range(&self) -> &Range<u64> {
        &self.range
    }

    /// Returns the emitted span's category.
    #[must_use]
    pub const fn kind(&self) -> XmlRenderKind {
        self.kind
    }

    /// Returns whether the span is proved by its contiguous source ranges alone.
    /// Consumers that also verify the child-absence evidence can complete those spans.
    /// A start tag's attribute-name choices share the same line's attribute-name observations.
    #[must_use]
    pub const fn complete(&self) -> bool {
        self.complete
    }

    /// Returns a complete child-absence proof when this span also needs a parent-word survey.
    #[must_use]
    pub const fn child_absence(&self) -> Option<&XmlChildAbsence> {
        match &self.sources {
            Contributors::ChildAbsence { absence, .. } => Some(absence),
            Contributors::Fields(_) | Contributors::Indentation(_) => None,
        }
    }

    /// Iterates the contiguous contributors other than the separate child-absence proof.
    pub fn sources(&self) -> impl Iterator<Item = &XmlRawSpan> {
        let (fields, indent): (&[_], &[_]) = match &self.sources {
            Contributors::Fields(fields) | Contributors::ChildAbsence { fields, .. } => {
                (fields, &[])
            }
            Contributors::Indentation(indent) => (&[], indent),
        };
        fields.iter().flatten().chain(indent)
    }
}
