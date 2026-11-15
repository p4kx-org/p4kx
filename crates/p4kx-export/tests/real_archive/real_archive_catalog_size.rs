// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Measures the normal catalog encoder against both configured installed Archives.

use std::error::Error;
use std::io;
use std::path::Path;
use std::time::{Instant, UNIX_EPOCH};

use p4kx_channels::channel_name;
use p4kx_export::{CatalogError, CatalogOptions, CatalogSummary, Compression, write_catalog_from};
use p4kx_extraction::{Archive, ArchiveKey, ArchiveWatch, Build, DataForgeEntry};
use p4kx_testkit::{real_archive_key, real_archive_pair};

use crate::real_archive_verification::print_line;

#[path = "real_archive_catalog_section_pipe.rs"]
mod sections;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn normal_catalog_sizes_for_installed_pair() -> TestResult {
    let Some((live, ptu)) = real_archive_pair()? else {
        print_line("catalog sizing skipped: configure the second installed Archive");
        return Ok(());
    };
    let live_build = installed_build(&live, "LIVE")?;
    let ptu_build = installed_build(&ptu, "PTU")?;
    if live_build == ptu_build {
        return Err("catalog sizing needs distinct LIVE and PTU Builds".into());
    }
    let key = real_archive_key()?;
    let failures: Vec<_> = [("LIVE", live, live_build), ("PTU", ptu, ptu_build)]
        .into_iter()
        .filter_map(|(channel, path, build)| measure(&path, channel, &build, &key).err())
        .map(|failure| failure.to_string())
        .collect();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n").into())
    }
}

fn installed_build(path: &Path, expected_channel: &str) -> TestResult<Build> {
    if !channel_name(path).is_some_and(|channel| channel.eq_ignore_ascii_case(expected_channel)) {
        return Err(format!("expected an installed {expected_channel} Archive").into());
    }
    let parent = path
        .parent()
        .ok_or("installed Archive directory unavailable")?;
    let build = Build::read_from(parent)
        .map_err(|_| format!("{expected_channel} Build manifest read failed"))?
        .ok_or_else(|| format!("{expected_channel} Build manifest missing"))?;
    if [
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    ]
    .iter()
    .any(|value| value.trim().is_empty())
    {
        return Err(format!("{expected_channel} Build identity is incomplete").into());
    }
    Ok(build)
}

fn measure(path: &Path, channel: &str, build: &Build, key: &ArchiveKey) -> TestResult {
    let archive = Archive::open_with_key(path, Some(key.clone()))
        .map_err(|_| format!("{channel} Archive open failed"))?;
    if archive.build() != Some(build) || archive.file_modified().is_none() {
        return Err(format!("{channel} Archive identity or timestamp is unavailable").into());
    }
    let watch = ArchiveWatch::new(&archive);
    watch
        .check()
        .map_err(|_| format!("{channel} input changed before catalog sizing"))?;
    let change = build.change_number();
    print_line(&format!(
        "catalog sizing input={channel} change={change} archive_bytes={} archive_entries={}",
        archive.file_len(),
        archive.entries().len(),
    ));
    let started = Instant::now();
    let entry =
        DataForgeEntry::read(&archive).map_err(|_| format!("{channel} DataForge read failed"))?;
    if !entry.origin().matches_archive(&archive) {
        return Err(format!("{channel} DataForge source does not match the opened Archive").into());
    }
    let options = CatalogOptions::new(UNIX_EPOCH);
    let result = measure_writer(&archive, &entry, &options);
    let elapsed = started.elapsed();
    watch
        .check()
        .map_err(|_| format!("{channel} input changed during catalog sizing"))?;
    let (summary, census) = result.map_err(|error| {
        let reason = match error.downcast_ref::<CatalogError>() {
            Some(CatalogError::IncompleteExport(_)) => {
                "normal publication refused by export health"
            }
            None => "normal catalog section census failed",
            _ => "normal catalog generation failed",
        };
        format!(
            "{channel} change={change}: {reason} after {} ms",
            elapsed.as_millis()
        )
    })?;
    if !summary.is_complete() {
        return Err(format!("{channel} catalog sizing requires complete export health").into());
    }
    let encoded = summary
        .compressed_size()
        .ok_or("catalog sizing requires a completed Zstd frame")?;
    print_line(&format!(
        "catalog sizing input={channel} change={change} decoded_bytes={} encoded_bytes={encoded} elapsed_ms={}",
        summary.size(),
        elapsed.as_millis()
    ));
    print_census(channel, change, &summary, census.as_ref())
}

fn print_census(
    channel: &str,
    change: &str,
    summary: &CatalogSummary,
    census: Option<&sections::Census>,
) -> TestResult {
    if let Some(census) = census {
        census
            .reconcile(summary)
            .map_err(|error| format!("{channel} change={change}: {error}"))?;
        print_line(&format!(
            "catalog census input={channel} change={change} {}",
            serde_json::to_string(census)?
        ));
    }
    Ok(())
}

fn measure_writer(
    archive: &Archive,
    entry: &DataForgeEntry,
    options: &CatalogOptions<'_>,
) -> TestResult<(CatalogSummary, Option<sections::Census>)> {
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
