// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;

use p4kx_extraction::field_statistics_report::{
    FieldStatisticsAddressedOrigins, FieldStatisticsReport,
};
use p4kx_extraction::{FieldOrigins, FieldStat, MAX_DISTINCT_VALUES, Number, SourceInventory};
use serde::Serialize;

use crate::dcb_fields::Listing;
use crate::dcb_fields_source_proof::SourceProof;
use crate::json;

/// Writes the listing as one line of JSON, defined in docs/cli-json.md.
pub(crate) fn write(
    out: &mut impl Write,
    archive: &Path,
    listing: &Listing<'_>,
    report: &FieldStatisticsReport<'_>,
    displayed: &FieldStatisticsAddressedOrigins,
) -> io::Result<()> {
    let result = ResultJson {
        archive: json::path(archive),
        records: listing.statistics.records(),
        unreadable_records: listing.statistics.unreadable_records(),
        unreadable_values: listing.statistics.unreadable_values(),
        max_distinct: MAX_DISTINCT_VALUES,
        field_count: listing.fields.len(),
        fields: listing
            .fields
            .iter()
            .map(|field| FieldJson::of(field))
            .collect(),
        field_origins: &displayed.fields,
        provenance: &report.inventory,
        source_proof: SourceProof::of(report, displayed),
    };
    serde_json::to_writer(&mut *out, &result)?;
    writeln!(out)
}

#[derive(Serialize)]
struct ResultJson<'a> {
    archive: String,
    records: u64,
    unreadable_records: u64,
    unreadable_values: u64,
    max_distinct: usize,
    field_count: usize,
    fields: Vec<FieldJson<'a>>,
    #[serde(rename = "fieldOrigins")]
    field_origins: &'a FieldOrigins,
    provenance: &'a SourceInventory,
    #[serde(rename = "sourceProof")]
    source_proof: SourceProof<'a>,
}

#[derive(Serialize)]
struct FieldJson<'a> {
    path: &'a str,
    occurrences: u64,
    records: u64,
    empty: u64,
    kinds: Vec<KindJson>,
    distinct: usize,
    distinct_capped: bool,
    min: Option<NumberJson>,
    max: Option<NumberJson>,
}

#[derive(Serialize)]
struct KindJson {
    kind: String,
    count: u64,
}

#[derive(Serialize)]
#[serde(untagged)]
enum NumberJson {
    Signed(i64),
    Unsigned(u64),
    Float(f64),
}

impl<'a> FieldJson<'a> {
    fn of(field: &'a FieldStat) -> Self {
        Self {
            path: field.path(),
            occurrences: field.occurrences(),
            records: field.records(),
            empty: field.empty(),
            kinds: field
                .kinds()
                .iter()
                .map(|(kind, count)| KindJson {
                    kind: kind.to_string(),
                    count: *count,
                })
                .collect(),
            distinct: field.distinct(),
            distinct_capped: field.distinct_capped(),
            min: field.min().and_then(NumberJson::of),
            max: field.max().and_then(NumberJson::of),
        }
    }
}

impl NumberJson {
    /// Returns the number as JSON, or `None` for an infinite float, which JSON cannot hold.
    fn of(number: Number) -> Option<Self> {
        match number {
            Number::Integer(value) => i64::try_from(value)
                .map(Self::Signed)
                .or_else(|_| u64::try_from(value).map(Self::Unsigned))
                .ok(),
            other => other
                .to_string()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .map(Self::Float),
        }
    }
}
