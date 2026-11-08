// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reconciliation of independently read shader sources with actual exporter counters.

use std::path::Path;

use p4kx_extraction::ArchiveWatch;
use p4kx_media::{Index, MediaKind, MediaRecord};

use super::TestResult;
use super::shader_accounting::require;
use super::shader_artifacts::{self, Retained};
use super::shader_expected::ExpectedSource;
use super::shader_records::Observed;

pub(super) const MAX_RECORDS: u64 = 100_000;

#[derive(Default)]
pub(super) struct Report {
    pub(super) definitions: u64,
    pub(super) properties: u64,
    pub(super) failed: u64,
    pub(super) partial: u64,
}

pub(super) fn verify(
    output: &Path,
    index: &Index,
    expected: &[ExpectedSource],
    retained: &[Retained],
    enumerated: u64,
    watch: &ArchiveWatch,
) -> TestResult<Report> {
    require(index.enumeration_complete, "source inventory is incomplete")?;
    require(
        index.kinds.len() == 1,
        "shader-only verification selected another provider",
    )?;
    let kind = index
        .kinds
        .first()
        .ok_or("shader provider summary missing")?;
    require(
        kind.provider.kind == MediaKind::Shader,
        "selected provider is not shader",
    )?;
    let bytes = shader_artifacts::checked(
        &output.join(&index.artifact_base),
        &kind.artifact,
        shader_artifacts::OUTPUT_BYTES,
    )?;
    let validator = jsonschema::validator_for(&p4kx_media::shader_record_schema()?)?;
    let mut observed: Vec<_> = expected.iter().map(|_| Observed::default()).collect();
    let mut rows = 0_u64;
    for line in std::str::from_utf8(&bytes)?.lines() {
        watch.poll()?;
        rows += 1;
        require(
            rows <= MAX_RECORDS,
            "shader row verification budget exceeded",
        )?;
        let record: MediaRecord = serde_json::from_str(line)?;
        validator
            .validate(&serde_json::to_value(&record)?)
            .map_err(|error| error.to_string())?;
        require(
            record.kind == MediaKind::Shader,
            "non-shader record in shader stream",
        )?;
        let source = expected
            .iter()
            .position(|source| source.identity == record.source.identity)
            .ok_or("row has unknown physical source")?;
        observed
            .get_mut(source)
            .ok_or("source accumulator missing")?
            .record(
                &record,
                expected.get(source).ok_or("source expectation missing")?,
                retained.get(source).ok_or("retained source missing")?,
            )?;
    }
    for (actual, source) in observed.iter().zip(expected) {
        actual.finish(source)?;
    }
    reconcile(index, expected, &observed, enumerated, rows)
}

fn reconcile(
    index: &Index,
    expected: &[ExpectedSource],
    observed: &[Observed],
    enumerated: u64,
    rows: u64,
) -> TestResult<Report> {
    let report = Report {
        definitions: observed.iter().map(|source| source.definitions).sum(),
        properties: observed.iter().map(|source| source.properties).sum(),
        failed: observed.iter().map(|source| source.failures).sum(),
        partial: observed.iter().map(|source| source.partial).sum(),
    };
    physical_counts(index, u64::try_from(expected.len())?, enumerated)?;
    record_counts(index, observed, &report, rows)?;
    Ok(report)
}

fn physical_counts(index: &Index, selected: u64, enumerated: u64) -> TestResult {
    let kind = index
        .kinds
        .first()
        .ok_or("shader provider summary missing")?;
    require(
        index.source_entries.enumerated == enumerated,
        "physical enumeration accounting differs",
    )?;
    require(
        index.source_entries.written == selected
            && index.source_entries.failed == 0
            && index.source_entries.partial == 0,
        "retained physical-source accounting differs",
    )?;
    require(
        index.source_entries.skipped
            == enumerated
                .checked_sub(selected)
                .ok_or("source count underflow")?,
        "source Filter accounting differs",
    )?;
    require(
        kind.counts.source_entries.enumerated == selected
            && kind.counts.source_entries.skipped == 0,
        "provider source callback accounting differs",
    )?;
    require(
        kind.counts.converted_outputs.enumerated == 0,
        "shader metadata was counted as converted output",
    )
}

fn record_counts(index: &Index, observed: &[Observed], report: &Report, rows: u64) -> TestResult {
    let kind = index
        .kinds
        .first()
        .ok_or("shader provider summary missing")?;
    let complete = observed.iter().map(|source| source.complete).sum::<u64>();
    require(
        kind.counts.records.enumerated == rows && kind.counts.records.written == complete,
        "record counting units differ",
    )?;
    require(
        kind.counts.records.failed == report.failed
            && kind.counts.records.partial == report.partial
            && kind.counts.records.skipped == 0,
        "record failure/partial accounting differs",
    )?;
    let selected = u64::try_from(observed.len())?;
    let failed_sources = u64::try_from(
        observed
            .iter()
            .filter(|source| source.failures != 0)
            .count(),
    )?;
    let partial_sources = u64::try_from(
        observed
            .iter()
            .filter(|source| source.failures == 0 && source.partial != 0)
            .count(),
    )?;
    require(
        kind.counts.source_entries.failed == failed_sources
            && kind.counts.source_entries.partial == partial_sources,
        "source outcome accounting differs",
    )?;
    require(
        kind.counts.source_entries.written == selected - failed_sources - partial_sources,
        "complete source count differs",
    )?;
    require(
        index.complete == (report.failed == 0 && report.partial == 0),
        "index completeness conceals projection loss",
    )
}
