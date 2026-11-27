// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::ScalarSource;

/// Original decoder operands retained with a physical field occurrence.
///
/// A single scalar consults at most one enum bound, one text bound and one text extent.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FieldStatisticsScalarRead {
    enum_bound: bool,
    text_bound: bool,
    text: Option<ScalarSource>,
}

impl FieldStatisticsScalarRead {
    pub(super) const fn observe(&mut self, source: ScalarSource) {
        match source {
            ScalarSource::EnumCount => self.enum_bound = true,
            ScalarSource::TextLength => self.text_bound = true,
            _ => self.text = Some(source),
        }
    }

    /// Returns the original decoder facts associated with this occurrence.
    pub fn sources(self) -> impl Iterator<Item = ScalarSource> {
        [
            self.enum_bound.then_some(ScalarSource::EnumCount),
            self.text_bound.then_some(ScalarSource::TextLength),
            self.text,
        ]
        .into_iter()
        .flatten()
    }
}
