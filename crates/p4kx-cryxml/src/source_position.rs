// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

/// The original coordinate of an element in its owning XML source.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SourceElementPosition {
    /// The complete element's half-open byte range in the original text input.
    Text(Range<usize>),
    /// The original zero-based CryXml node table index.
    CryXml(u32),
}

/// The original coordinate of an attribute in its owning XML source.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SourceAttributePosition {
    /// The original attribute spelling and quoted value as a half-open byte range.
    Text(Range<usize>),
    /// The node and attribute table positions of the stored CryXml declaration.
    CryXml {
        /// Original owning node table index.
        node_index: u32,
        /// Original attribute table index.
        attribute_index: u32,
    },
}

pub(crate) const fn original_range(range: Range<usize>, prefix_bytes: usize) -> Range<usize> {
    (range.start + prefix_bytes)..(range.end + prefix_bytes)
}
