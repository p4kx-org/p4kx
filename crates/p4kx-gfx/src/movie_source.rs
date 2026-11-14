// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A byte range in one explicitly identified movie coordinate domain.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct MovieSpan {
    /// Zero-based starting byte.
    pub offset: usize,
    /// Number of bytes in the range.
    pub length: usize,
}

impl MovieSpan {
    pub(crate) const fn new(offset: usize, end: usize) -> Self {
        Self {
            offset,
            length: end - offset,
        }
    }

    /// Returns the first byte after this range.
    #[must_use]
    pub const fn end(self) -> usize {
        self.offset + self.length
    }
}

/// Locations of the retained header fields and of the input used for decompression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct MovieSource {
    /// File signature in original movie input bytes.
    pub signature: MovieSpan,
    /// File version in original movie input bytes.
    pub version: MovieSpan,
    /// Declared length in original movie input bytes.
    pub declared_length: MovieSpan,
    /// Bytes supplied to the movie reader, before movie decompression.
    pub input_length: usize,
    /// Original bytes actually consumed by the movie decompressor.
    pub compressed: Option<MovieSpan>,
    /// Bit-packed stage rectangle in logical decoded movie bytes.
    pub stage: MovieSpan,
    /// Fixed-point frame rate in logical decoded movie bytes.
    pub frame_rate: MovieSpan,
    /// Declared frame count in logical decoded movie bytes.
    pub frame_count: MovieSpan,
}

/// The admitted header and body of one physical tag occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct TagSite {
    /// Tag code as stored in the header.
    pub code: u16,
    /// Short or long tag header in logical decoded movie bytes.
    pub header: MovieSpan,
    /// Tag body in logical decoded movie bytes.
    pub body: MovieSpan,
}

/// One physical row of a tag that declares several items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct TagItemSite {
    /// The owning tag.
    pub tag: TagSite,
    /// Zero-based authored row within that tag.
    pub ordinal: usize,
}

impl TagItemSite {
    pub(crate) const fn new(tag: TagSite, ordinal: usize) -> Self {
        Self { tag, ordinal }
    }
}
