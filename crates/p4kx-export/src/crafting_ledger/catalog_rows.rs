// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Filter;

use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::selection::{self, Kind};

/// The catalog selection applied to rows after their source was canonically read.
pub(super) struct Selection<'s> {
    filter: Option<&'s Filter>,
    ledger: &'s mut ExportLedger,
}

impl<'s> Selection<'s> {
    pub(super) const fn new(filter: Option<&'s Filter>, ledger: &'s mut ExportLedger) -> Self {
        Self { filter, ledger }
    }

    pub(super) fn matches(&self, path: &str) -> bool {
        self.filter.is_none_or(|filter| filter.matches(path))
    }

    pub(super) fn entities<T, E>(
        &mut self,
        kind: Kind,
        items: &[T],
        names: impl Fn(&T) -> (&str, &str),
        make: impl Fn(&T) -> E,
        guid: impl Fn(&E) -> CatalogGuid,
    ) -> (Vec<E>, usize) {
        let mut built = Vec::with_capacity(items.len());
        let mut skipped = 0_usize;
        for item in items {
            let (path, name) = names(item);
            if !self.matches(path) {
                continue;
            }
            if selection::is_kind_leftover(kind, path, name) {
                skipped = skipped.saturating_add(1);
                self.ledger.skipped_rows(kind, 1);
            } else {
                built.push(make(item));
            }
        }
        built.sort_by_cached_key(|entity| guid(entity).text());
        (built, skipped)
    }
}

#[cfg(test)]
#[path = "catalog_rows_tests.rs"]
mod tests;
