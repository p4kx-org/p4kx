// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Measures the normal catalog encoder against both configured installed Archives.

use std::io;

use p4kx_export::{
    CatalogError, CatalogOptions, CatalogStage, CatalogSummary, Compression, write_catalog_from,
};
use p4kx_extraction::{Archive, DataForgeEntry};

#[path = "real_archive_catalog_artifact.rs"]
pub(crate) mod artifact;
#[path = "real_archive_catalog_replay.rs"]
mod replay;
#[path = "real_archive_catalog_section_pipe.rs"]
mod sections;
#[path = "real_archive_catalog_size_support.rs"]
mod support;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn normal_catalog_sizes_for_installed_pair() -> support::TestResult {
    support::measure_pair(measure_writer, print_census, None)
}

fn print_census(
    channel: &str,
    change: &str,
    summary: &CatalogSummary,
    census: Option<sections::Census>,
) -> support::TestResult {
    if let Some(census) = census {
        census
            .reconcile(summary)
            .map_err(|error| format!("{channel} change={change}: {error}"))?;
        support::print_line(&format!(
            "catalog census input={channel} change={change} {}",
            serde_json::to_string(&census)?
        ));
    }
    Ok(())
}

fn measure_writer(
    archive: &Archive,
    entry: &DataForgeEntry,
    options: &CatalogOptions<'_>,
    _report: &dyn Fn(CatalogStage),
) -> support::TestResult<(CatalogSummary, Option<sections::Census>)> {
    if std::env::var_os("P4KX_CATALOG_SECTION_CENSUS").is_some_and(|value| value == "1") {
        let (summary, census) = sections::transfer(|writer| {
            write_catalog_from(archive, entry, options, Compression::ZSTD, || Ok(writer))
        })?;
        Ok((summary, Some(census)))
    } else {
        let (summary, _) =
            write_catalog_from(
                archive,
                entry,
                options,
                Compression::ZSTD,
                || Ok(io::sink()),
            )?;
        Ok((summary, None))
    }
}
