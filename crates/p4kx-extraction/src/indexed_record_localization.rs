// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;
use std::ops::Range;

use p4kx_dataforge::{Resolution, TextResolver};
use p4kx_localization::{LocalizationError, is_placeholder_key};

use super::display;
use super::record_localization::LocalizationLookupEvidence;
use crate::localization::offset_of;
use crate::{LocalizationEntry, LocalizationLoadError, SourceEvidenceError, SourceUse, SourceUses};

/// An owned index of the selected values in one original language Entry.
#[derive(Debug)]
pub struct IndexedRecordLocalization {
    entry: LocalizationEntry,
    rows: Vec<IndexedRow>,
    complete_absence: bool,
}

impl IndexedRecordLocalization {
    /// Builds a lookup index over the retained original language Entry.
    ///
    /// # Errors
    /// Returns a localization indexing error.
    pub fn try_new(entry: LocalizationEntry) -> Result<Self, LocalizationLoadError> {
        let ini = entry.index()?;
        let complete_absence = !entry.text().is_lossy() && ini.skipped() == 0;
        let text = entry.text().text();
        let mut rows = Vec::new();
        rows.try_reserve_exact(ini.len())
            .map_err(|_| LocalizationLoadError::Index {
                path: entry.path().to_owned(),
                source: LocalizationError::OutOfMemory { keys: ini.len() },
            })?;
        let range = |part| {
            range_of(text, part).ok_or_else(|| LocalizationLoadError::IndexOutsideInput {
                path: entry.path().to_owned(),
            })
        };
        for (key, value) in ini.iter() {
            rows.push(IndexedRow {
                key: range(key)?,
                value: range(value)?,
            });
        }
        rows.sort_unstable_by(|left, right| key_order(left.key(text), right.key(text)));
        Ok(Self {
            entry,
            rows,
            complete_absence,
        })
    }

    /// Appends the original language lookup and selected value for an observed Record key.
    ///
    /// # Errors
    /// Returns an error if the key selector cannot be bound to the source-use inventory.
    pub fn append_lookup_uses(
        &self,
        key: &str,
        registry: &SourceUses,
        authored_key: &SourceUse,
        output: &mut Vec<SourceUse>,
    ) -> Result<bool, SourceEvidenceError> {
        if is_placeholder_key(key) {
            return Ok(false);
        }
        LocalizationLookupEvidence::new(&self.entry, self.lookup(key), self.complete_absence)
            .append_uses(registry, Some(authored_key), output)
    }

    fn lookup(&self, key: &str) -> Option<&str> {
        let key = key.trim_prefix('@');
        let text = self.entry.text().text();
        let matching = self
            .rows
            .binary_search_by(|row| key_order(row.key(text), key))
            .ok()?;
        self.rows
            .get(matching)
            .and_then(|row| text.get(row.value.clone()))
    }
}

impl TextResolver for IndexedRecordLocalization {
    fn resolve<'resolver>(&'resolver self, key: &str) -> Resolution<'resolver> {
        if is_placeholder_key(key) {
            Resolution::Keep
        } else {
            display(self.lookup(key))
        }
    }
}

#[derive(Debug)]
struct IndexedRow {
    key: Range<usize>,
    value: Range<usize>,
}

impl IndexedRow {
    fn key<'text>(&self, text: &'text str) -> &'text str {
        text.get(self.key.clone()).unwrap_or_default()
    }
}

fn range_of(text: &str, part: &str) -> Option<Range<usize>> {
    let start = usize::try_from(offset_of(text, part)?).ok()?;
    Some(start..start.checked_add(part.len())?)
}

fn key_order(left: &str, right: &str) -> Ordering {
    left.bytes()
        .map(|byte| byte.to_ascii_lowercase())
        .cmp(right.bytes().map(|byte| byte.to_ascii_lowercase()))
}
