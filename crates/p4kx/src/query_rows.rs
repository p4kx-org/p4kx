// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_extraction::query::{QueryRun, Row};
use p4kx_extraction::{QUERY_PAGE_SIZE, Value};

/// The number of rows written, of Records whose row failed and of the unresolved References in
/// the rows written.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Counts {
    pub(crate) records: u64,
    pub(crate) failed: u64,
    pub(crate) unresolved: u64,
}

impl Counts {
    /// Returns `<n> Records<after>`, followed by `, <m> failed` when a Record failed and by
    /// `, <k> unresolved References` when `->` met a null or dangling Reference.
    pub(crate) fn line(&self, after: &str) -> String {
        let noun = if self.records == 1 {
            "Record"
        } else {
            "Records"
        };
        let failed = if self.failed == 0 {
            String::new()
        } else {
            format!(", {} failed", self.failed)
        };
        let unresolved = match self.unresolved {
            0 => String::new(),
            1 => ", 1 unresolved Reference".to_owned(),
            count => format!(", {count} unresolved References"),
        };
        format!("{} {noun}{after}{failed}{unresolved}", self.records)
    }
}

/// Calls `write` with each row's record path and values, page by page, and reports on stderr
/// each Record whose values cannot be read; `counts` holds what was done also after an error.
///
/// # Errors
///
/// Returns the [`io::Error`] of `write`; a report that cannot be written to stderr is dropped.
pub(crate) fn for_each_row(
    run: &QueryRun<'_, '_>,
    counts: &mut Counts,
    mode: RowMode,
    mut write: impl FnMut(&Row<'_>, &[Option<Value<'_>>]) -> io::Result<()>,
) -> io::Result<()> {
    let pages = match mode {
        RowMode::Raw => run.pages(QUERY_PAGE_SIZE),
        RowMode::Observed => run.pages_observed(QUERY_PAGE_SIZE),
    };
    for page in pages {
        for row in &page {
            if let Ok(cells) = row.cells() {
                write(row, cells)?;
                counts.records = counts.records.saturating_add(1);
                counts.unresolved = counts
                    .unresolved
                    .saturating_add(u64::try_from(row.unresolved_references()).unwrap_or(u64::MAX));
            } else {
                counts.failed = counts.failed.saturating_add(1);
                report_failed(row);
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(crate) enum RowMode {
    Raw,
    Observed,
}

/// Reports on stderr that the values of `row`'s Record cannot be read; a report that cannot be
/// written is dropped.
pub(crate) fn report_failed(row: &Row<'_>) {
    if let Err(error) = row.cells() {
        let _unreported = writeln!(
            io::stderr().lock(),
            "failed {} ({}): {error}",
            row.record_path(),
            row.record().guid()
        );
    }
}
