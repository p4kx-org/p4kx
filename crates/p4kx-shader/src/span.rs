// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// Half-open byte offsets from the beginning of the original decoded Entry payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Span {
    /// First included byte.
    pub start: usize,
    /// First excluded byte.
    pub end_exclusive: usize,
}

impl Span {
    pub(crate) const fn new(start: usize, end_exclusive: usize) -> Self {
        Self {
            start,
            end_exclusive,
        }
    }

    /// Returns the source bytes when the offsets lie within the supplied payload.
    #[must_use]
    pub fn bytes(self, source: &[u8]) -> Option<&[u8]> {
        source.get(self.start..self.end_exclusive)
    }

    /// Returns source text when both offsets lie on UTF-8 boundaries.
    #[must_use]
    pub fn text(self, source: &str) -> Option<&str> {
        source.get(self.start..self.end_exclusive)
    }
}
