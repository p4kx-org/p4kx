// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Measures the normal catalog encoder with a phase observer and a discard sink.

use std::{env, io};

use p4kx_extraction::{Archive, DataForgeEntry, SourceUses};

use crate::options::{CatalogRetention, CatalogRun, CatalogStage};
use crate::{CatalogError, CatalogOptions, CatalogSummary, Compression};

#[path = "../tests/real_archive/real_archive_catalog_size_support.rs"]
mod support;

#[path = "build_catalog_retention_tests.rs"]
mod retention_tests;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn normal_catalog_size_phases_for_installed_pair() -> support::TestResult {
    support::measure_pair(measure_writer, |_, _, _, ()| Ok(()), Some(report_phase))
}

fn measure_writer(
    archive: &Archive,
    entry: &DataForgeEntry,
    options: &CatalogOptions<'_>,
    report: &dyn Fn(CatalogStage),
) -> support::TestResult<(CatalogSummary, ())> {
    let observation = CatalogRun::new(options, Some(report));
    let census = env::var_os("P4KX_CATALOG_PACKED_KEY_CENSUS").is_some_and(|value| value == "1");
    let retention = |sample: CatalogRetention, uses: &SourceUses| {
        report_retention(sample);
        if census && sample.stage == CatalogStage::ExportHealth {
            report_packed_keys(observation, uses, sample.packed_source_uses)?;
        }
        Ok(())
    };
    let run = observation.with_retention(&retention);
    let (summary, _) =
        super::write_catalog_from_with_run(archive, entry, run, Compression::ZSTD, || {
            Ok(io::sink())
        })?;
    Ok((summary, ()))
}

#[expect(
    clippy::print_stderr,
    reason = "retained counts accompany the external process memory samples"
)]
fn report_retention(sample: CatalogRetention) {
    eprintln!(
        "catalog retained: stage={} source_uses={} field_inspections={} inspection_capacity={} inline_inspection_bytes={} packed_source_uses={}",
        sample.stage.as_str(),
        sample.source_uses,
        sample.field_inspections,
        sample.inspection_capacity,
        sample.inline_inspection_bytes,
        sample.packed_source_uses,
    );
}

#[expect(
    clippy::print_stderr,
    reason = "the opt-in native sizing witness records aggregate key storage"
)]
fn report_packed_keys(
    run: CatalogRun<'_, '_>,
    uses: &SourceUses,
    expected_rows: usize,
) -> Result<(), CatalogError> {
    let totals = uses.packed_key_storage_census(|| run.check_cancelled())?;
    assert_eq!(
        totals.iter().map(|(rows, _)| rows).sum::<usize>(),
        expected_rows,
        "the census visits every packed key",
    );
    let buckets = [
        ("direct", "registry-only"),
        ("direct", "shared"),
        ("typed", "registry-only"),
        ("typed", "shared"),
    ];
    for ((site, ownership), (rows, utf8_bytes)) in buckets.into_iter().zip(totals) {
        eprintln!(
            "catalog packed keys: stage=export-health site={site} ownership={ownership} rows={rows} utf8_bytes={utf8_bytes}",
        );
    }
    Ok(())
}

#[expect(
    clippy::print_stderr,
    reason = "phase events delimit the external process memory samples"
)]
fn report_phase(channel: &str, change: &str, stage: CatalogStage) {
    eprintln!(
        "catalog sizing input={channel} change={change} phase={}",
        stage.as_str()
    );
}
