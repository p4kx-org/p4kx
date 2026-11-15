// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, DataForge, DataForgeEntry, DataForgeError, DataForgeLoadError};
use thiserror::Error;

use crate::{
    Found, Needle, ObservedSearch, SearchOptions, SearchReport, Source, Unavailable, entries,
    localization, search_records,
};

/// A search that could not run.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SearchError {
    /// The Archive's DataForge cannot be read.
    #[error(transparent)]
    DataForge(#[from] DataForgeLoadError),
    /// The Archive's DataForge cannot be opened.
    #[error("the DataForge cannot be opened")]
    DataForgeOpen(#[source] DataForgeError),
}

/// Searches `archive` in the sources of `options`, in the order Records, localisation, Entries.
///
/// The DataForge is read for the search of Records and dropped with it. An Archive without a
/// DataForge adds [`Unavailable`] to the report instead of failing. Returns `Ok(None)` once `stop`
/// has returned `true`.
///
/// # Errors
///
/// Returns a [`SearchError`] when the DataForge exists but cannot be read, e.g. because it is
/// encrypted and the Archive has no key.
pub fn search(
    archive: &Archive,
    needle: &Needle,
    options: &SearchOptions,
    stop: &(dyn Fn() -> bool + Sync),
) -> Result<Option<SearchReport>, SearchError> {
    search_observed(archive, needle, options, stop)
        .map(|result| result.map(|observed| observed.report))
}

/// Searches an Archive and keeps its actual returned hit origins.
///
/// # Errors
///
/// Returns the same DataForge read or open errors as [`search`].
pub fn search_observed<'archive>(
    archive: &'archive Archive,
    needle: &Needle,
    options: &SearchOptions,
    stop: &(dyn Fn() -> bool + Sync),
) -> Result<Option<ObservedSearch<'archive>>, SearchError> {
    let entry = if options.sources.contains(Source::Records) {
        match DataForgeEntry::read(archive) {
            Ok(entry) => Some(entry),
            Err(DataForgeLoadError::Missing) => None,
            Err(error) => return Err(error.into()),
        }
    } else {
        None
    };
    search_observed_with_dataforge(archive, entry.as_ref(), needle, options, stop)
}

/// Searches using the supplied physical DataForge read and the Archive’s other sources.
///
/// # Errors
///
/// Returns a DataForge open error when a selected supplied database cannot be opened.
pub fn search_observed_with_dataforge<'archive>(
    archive: &'archive Archive,
    entry: Option<&DataForgeEntry>,
    needle: &Needle,
    options: &SearchOptions,
    stop: &(dyn Fn() -> bool + Sync),
) -> Result<Option<ObservedSearch<'archive>>, SearchError> {
    let entry = entry.filter(|_| options.sources.contains(Source::Records));
    let forge = entry
        .map(DataForgeEntry::open)
        .transpose()
        .map_err(SearchError::DataForgeOpen)?;
    let Some(mut result) = search_sources(archive, forge.as_ref(), needle, options, stop) else {
        return Ok(None);
    };
    if !result.report.records.is_empty()
        && let (Some(owner), Some(forge)) = (entry, forge.as_ref())
    {
        result.record_origin = owner.origin_of(forge).cloned();
        result.record_structures = result
            .report
            .records
            .iter()
            .map(|hit| {
                forge
                    .record(hit.record_index)
                    .and_then(|record| record.struct_type().ok())
                    .map(|structure| structure.index())
            })
            .collect();
    }
    Ok(Some(result))
}

/// Searches `archive` like [`search`] with a DataForge the caller already holds.
///
/// A window searches the one it keeps in memory. Without `forge` the search of Records adds
/// [`Unavailable`] to the report. Returns `None` once `stop` has returned `true`.
#[must_use]
pub fn search_in<'f>(
    archive: &Archive,
    forge: Option<&'f DataForge<'f>>,
    needle: &Needle,
    options: &SearchOptions,
    stop: &(dyn Fn() -> bool + Sync),
) -> Option<SearchReport> {
    search_sources(archive, forge, needle, options, stop).map(|result| result.report)
}

fn search_sources<'archive, 'forge>(
    archive: &'archive Archive,
    forge: Option<&'forge DataForge<'forge>>,
    needle: &Needle,
    options: &SearchOptions,
    stop: &(dyn Fn() -> bool + Sync),
) -> Option<ObservedSearch<'archive>> {
    let mut result = ObservedSearch::default();
    let report = &mut result.report;
    if options.sources.contains(Source::Records) {
        match forge {
            Some(forge) => {
                let found = search_records(forge, needle, options, stop)?;
                take(report, Source::Records, found, |report| &mut report.records);
            }
            None => report.unavailable.push(Unavailable {
                source: Source::Records,
                reason: DataForgeLoadError::Missing.to_string(),
            }),
        }
    }
    if options.sources.contains(Source::Localization) {
        let localized = localization::observed(archive, needle, options, stop)?;
        result.localization_origins = localized.origins;
        result.localization_lines = localized
            .found
            .hits
            .iter()
            .map(|hit| hit.line_index)
            .collect();
        take(
            report,
            Source::Localization,
            localized.found.map_hits(|located| located.hit),
            |report| &mut report.localization,
        );
    }
    if options.sources.contains(Source::Entries) {
        let found = entries::observed(archive, needle, options, stop)?;
        result.entry_origins = found.hits.iter().map(|hit| hit.source).collect();
        take(
            report,
            Source::Entries,
            found.map_hits(|located| located.hit),
            |report| &mut report.entries,
        );
    }
    Some(result)
}

fn take<T>(
    report: &mut SearchReport,
    source: Source,
    found: Found<T>,
    hits: impl FnOnce(&mut SearchReport) -> &mut Vec<T>,
) {
    *hits(report) = found.hits;
    if found.has_more {
        report.has_more = report.has_more.with(source);
    }
    report.stats.merge(&found.stats);
}
