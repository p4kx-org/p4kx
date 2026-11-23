// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

/// Original DataForge declarations consumed by a validated layout traversal.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LayoutSource {
    /// One participating original Struct row in inheritance order.
    #[non_exhaustive]
    Structure {
        /// Physical Struct table position.
        index: usize,
        /// Stored parent word, including the no-parent sentinel.
        parent: u32,
        /// First physical property table position in the original own-field declaration.
        first_property: u16,
        /// Number of original own properties, including zero.
        property_count: u16,
        /// Stored instance byte extent.
        size: u32,
    },
    /// One physical Property examined in its inherited-first position.
    #[non_exhaustive]
    Property {
        /// Original Property definition position.
        index: usize,
        /// Position of the field in the actual effective layout view.
        position: u64,
        /// Original decoded name bytes when the span is nonempty.
        name: Option<Range<u64>>,
    },
    /// The original target word consulted by the predicate's property resolver.
    #[non_exhaustive]
    PropertyTarget {
        /// Physical Property definition position.
        property_index: usize,
    },
    /// One original Property whose name could not be decoded.
    #[non_exhaustive]
    UnreadablePropertyName {
        /// Physical Property definition position that holds the unreadable name offset.
        index: usize,
    },
    /// An original Struct name actually compared by a typed selector.
    #[non_exhaustive]
    TypeName {
        /// Physical Struct definition position.
        index: usize,
        /// Original name bytes, absent if the stored name could not be read.
        name: Option<Range<u64>>,
    },
    /// An original parent word consulted by a typed ancestry check.
    #[non_exhaustive]
    TypeParent {
        /// Original Struct position containing the parent word.
        index: usize,
        /// Stored parent word, including the no-parent sentinel.
        stored_parent: u32,
    },
    /// An ancestry check whose original chain could not be established.
    #[non_exhaustive]
    UnavailableAncestry {
        /// The unresolved position, which may not exist in the original table.
        index: usize,
    },
    /// One original Struct whose layout could not be selected.
    #[non_exhaustive]
    UnavailableLayout {
        /// Attempted Struct definition position.
        index: usize,
    },
}
