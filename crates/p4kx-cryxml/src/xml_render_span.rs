// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::XmlFieldKind;

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

pub(crate) type RawSpans = [Option<XmlRawSpan>; 4];

/// Emitted bytes and their original decoded-input ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlRenderSpan {
    kind: XmlRenderKind,
    range: Range<u64>,
    sources: RawSpans,
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
            sources,
            complete,
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

    /// Returns whether the span's contribution proof is complete.
    /// A start tag's attribute-name choices share the same line's attribute-name observations.
    #[must_use]
    pub const fn complete(&self) -> bool {
        self.complete
    }

    /// Iterates all captured original contributors.
    pub fn sources(&self) -> impl Iterator<Item = &XmlRawSpan> {
        self.sources.iter().flatten()
    }
}
