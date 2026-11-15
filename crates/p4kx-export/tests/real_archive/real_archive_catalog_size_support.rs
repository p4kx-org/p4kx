// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Verifies source identity and reports the normal encoder's measured sizes.

use std::error::Error;
use std::path::Path;
use std::time::{Instant, UNIX_EPOCH};

use p4kx_channels::channel_name;
use p4kx_extraction::{Archive, ArchiveKey, ArchiveWatch, Build, DataForgeEntry};
use p4kx_testkit::{real_archive_key, real_archive_pair};

use super::{CatalogError, CatalogOptions, CatalogStage, CatalogSummary};

pub(super) type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(super) type PhaseObserver = fn(&str, &str, CatalogStage);

pub(super) fn measure_pair<T>(
    write: impl Fn(
        &Archive,
        &DataForgeEntry,
        &CatalogOptions<'_>,
        &dyn Fn(CatalogStage),
    ) -> TestResult<(CatalogSummary, T)>,
    report: impl Fn(&str, &str, &CatalogSummary, T) -> TestResult,
    phase: Option<PhaseObserver>,
) -> TestResult {
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
    let mut failures = Vec::new();
    for (channel, path, build) in [("LIVE", live, live_build), ("PTU", ptu, ptu_build)] {
        let result = measure(&path, channel, &build, &key, &write, phase)
            .and_then(|(summary, extra)| report(channel, build.change_number(), &summary, extra));
        if let Err(error) = result {
            failures.push(error.to_string());
        }
    }
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

fn measure<T>(
    path: &Path,
    channel: &str,
    build: &Build,
    key: &ArchiveKey,
    write: impl FnOnce(
        &Archive,
        &DataForgeEntry,
        &CatalogOptions<'_>,
        &dyn Fn(CatalogStage),
    ) -> TestResult<(CatalogSummary, T)>,
    phase: Option<PhaseObserver>,
) -> TestResult<(CatalogSummary, T)> {
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
    let report = |stage| {
        if let Some(phase) = phase {
            phase(channel, change, stage);
        }
    };
    let started = Instant::now();
    report(CatalogStage::ReadDataForge);
    let entry =
        DataForgeEntry::read(&archive).map_err(|_| format!("{channel} DataForge read failed"))?;
    if !entry.origin().matches_archive(&archive) {
        return Err(format!("{channel} DataForge source does not match the opened Archive").into());
    }
    let options = CatalogOptions::new(UNIX_EPOCH);
    let result = write(&archive, &entry, &options, &report);
    let elapsed = started.elapsed();
    watch
        .check()
        .map_err(|_| format!("{channel} input changed during catalog sizing"))?;
    let (summary, extra) = result.map_err(|error| {
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
    Ok((summary, extra))
}

#[expect(
    clippy::print_stdout,
    reason = "the measurements are the result of the verification"
)]
pub(super) fn print_line(line: &str) {
    println!("{line}");
}
