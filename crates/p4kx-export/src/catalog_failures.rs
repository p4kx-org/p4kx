// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice;

use crate::catalog_read::original::CatalogFailureDetails;

/// Borrowed catalog failures sharing their report's immutable text storage.
#[derive(Debug, Clone, Copy)]
pub struct CatalogFailures<'report> {
    rows: &'report [CatalogFailureDetails],
}

/// A human failure view and the directly sourced cells that produced it.
///
/// ~~~compile_fail
/// use p4kx_export::CatalogFailureView;
/// fn replace_text<'report>(mut view: CatalogFailureView<'report>, foreign: &'report str) {
///     view.reason = foreign;
/// }
/// ~~~
#[derive(Debug, Clone, Copy)]
pub struct CatalogFailureView<'report> {
    details: &'report CatalogFailureDetails,
}

/// Iterates failures without expanding or retaining their shared strings.
#[derive(Debug, Clone)]
pub struct CatalogFailureIter<'report> {
    rows: slice::Iter<'report, CatalogFailureDetails>,
}

impl<'report> CatalogFailures<'report> {
    pub(crate) const fn new(rows: &'report [CatalogFailureDetails]) -> Self {
        Self { rows }
    }

    /// Returns the number of retained human failures.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.rows.len()
    }

    /// Returns whether no human failure was retained.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Borrows the first retained failure.
    #[must_use]
    pub fn first(&self) -> Option<CatalogFailureView<'report>> {
        self.get(0)
    }

    /// Borrows one failure without expanding a preceding prefix of the collection.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<CatalogFailureView<'report>> {
        self.rows.get(index).map(view)
    }

    /// Borrows each human value and its original source cells.
    #[must_use]
    pub fn iter(&self) -> CatalogFailureIter<'report> {
        CatalogFailureIter {
            rows: self.rows.iter(),
        }
    }
}

impl<'report> CatalogFailureView<'report> {
    /// Borrows the original canonical GUID text.
    #[must_use]
    pub fn guid(&self) -> &'report str {
        self.details.human_guid()
    }

    /// Borrows the normalized path, empty when its header is unreadable.
    #[must_use]
    pub fn record_path(&self) -> &'report str {
        self.details.human_path()
    }

    /// Borrows the retained human error and its source chain.
    #[must_use]
    pub fn reason(&self) -> &'report str {
        self.details.human_reason()
    }
    /// Returns the semantic occurrence cells without importing storage-aggregate origins.
    #[must_use]
    pub const fn details(&self) -> &'report CatalogFailureDetails {
        self.details
    }
}

impl<'report> Iterator for CatalogFailureIter<'report> {
    type Item = CatalogFailureView<'report>;

    fn next(&mut self) -> Option<Self::Item> {
        self.rows.next().map(view)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.rows.size_hint()
    }
}

impl DoubleEndedIterator for CatalogFailureIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.rows.next_back().map(view)
    }
}

impl ExactSizeIterator for CatalogFailureIter<'_> {}

impl<'report> IntoIterator for CatalogFailures<'report> {
    type Item = CatalogFailureView<'report>;
    type IntoIter = CatalogFailureIter<'report>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'report> IntoIterator for &CatalogFailures<'report> {
    type Item = CatalogFailureView<'report>;
    type IntoIter = CatalogFailureIter<'report>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
const fn view(row: &CatalogFailureDetails) -> CatalogFailureView<'_> {
    CatalogFailureView { details: row }
}
