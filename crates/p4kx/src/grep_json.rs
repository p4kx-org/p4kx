// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use p4kx_extraction::{FieldOrigins, SourceInventory};
use p4kx_search::{Excerpt, ObservedSearch, SearchStats, Source};
use serde::Serialize;

use crate::json;

#[path = "grep_origins.rs"]
mod origins;

#[derive(Serialize)]
struct GrepJson<'a> {
    archive: String,
    text: &'a str,
    elapsed_ms: u64,
    records: Vec<RecordJson<'a>>,
    loc: Vec<LocJson<'a>>,
    entries: Vec<EntryJson<'a>>,
    more: Vec<&'static str>,
    unavailable: Vec<UnavailableJson<'a>>,
    stats: StatsJson,
    #[serde(rename = "fieldOrigins")]
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

#[derive(Serialize)]
struct RecordJson<'a> {
    record_path: &'a str,
    name: &'a str,
    #[serde(rename = "type")]
    type_name: &'a str,
    guid: String,
    field: &'a str,
    text: &'a str,
    #[serde(rename = "match")]
    matched: [usize; 2],
}

#[derive(Serialize)]
struct LocJson<'a> {
    language: &'static str,
    key: &'a str,
    text: &'a str,
    #[serde(rename = "match")]
    matched: [usize; 2],
}

#[derive(Serialize)]
struct EntryJson<'a> {
    path: &'a str,
    line: u64,
    offset: u64,
    text: &'a str,
    #[serde(rename = "match")]
    matched: [usize; 2],
}

#[derive(Serialize)]
struct UnavailableJson<'a> {
    source: &'static str,
    reason: &'a str,
}

#[derive(Serialize)]
struct StatsJson {
    records_scanned: u64,
    records_unreadable: u64,
    texts_scanned: u64,
    languages_scanned: u32,
    languages_unreadable: u32,
    entries_scanned: u64,
    entries_binary: u64,
    entries_too_large: u64,
    entries_key_required: u64,
    entries_unreadable: u64,
}

impl From<SearchStats> for StatsJson {
    fn from(stats: SearchStats) -> Self {
        Self {
            records_scanned: stats.records_scanned,
            records_unreadable: stats.records_unreadable,
            texts_scanned: stats.texts_scanned,
            languages_scanned: stats.languages_scanned,
            languages_unreadable: stats.languages_unreadable,
            entries_scanned: stats.entries_scanned,
            entries_binary: stats.entries_binary,
            entries_too_large: stats.entries_too_large,
            entries_key_required: stats.entries_key_required,
            entries_unreadable: stats.entries_unreadable,
        }
    }
}

fn span(excerpt: &Excerpt) -> [usize; 2] {
    let matched = excerpt.matched();
    [matched.start, matched.end]
}

/// Writes the report as one line of JSON, defined in `docs/cli-json.md`.
pub(crate) fn print(
    archive: &Path,
    text: &str,
    observed: &ObservedSearch<'_>,
    elapsed: Duration,
) -> Result<()> {
    let (field_origins, provenance) = origins::capture(observed)?;
    let report = observed.report();
    json::print(&GrepJson {
        archive: json::path(archive),
        text,
        elapsed_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        records: report
            .records
            .iter()
            .map(|hit| RecordJson {
                record_path: &hit.record_path,
                name: &hit.name,
                type_name: &hit.type_name,
                guid: hit.guid.to_string(),
                field: &hit.field,
                text: hit.excerpt.text(),
                matched: span(&hit.excerpt),
            })
            .collect(),
        loc: report
            .localization
            .iter()
            .map(|hit| LocJson {
                language: hit.language.code(),
                key: &hit.key,
                text: hit.excerpt.text(),
                matched: span(&hit.excerpt),
            })
            .collect(),
        entries: report
            .entries
            .iter()
            .map(|hit| EntryJson {
                path: &hit.path,
                line: hit.line,
                offset: hit.offset,
                text: hit.excerpt.text(),
                matched: span(&hit.excerpt),
            })
            .collect(),
        more: report.has_more.iter().map(Source::name).collect(),
        unavailable: report
            .unavailable
            .iter()
            .map(|missing| UnavailableJson {
                source: missing.source.name(),
                reason: &missing.reason,
            })
            .collect(),
        stats: report.stats.into(),
        field_origins,
        provenance,
    })?;
    Ok(())
}
