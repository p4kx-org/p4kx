// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Complete image sets produced while converting real Archive textures.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::Path;

use p4kx_extraction::{Archive, ConflictPolicy, Entry, ExtractOptions, ExtractionReport, extract};
use p4kx_testkit::real_archive_path;

use crate::TestResult;

#[path = "../support/texture_artifacts.rs"]
mod artifacts;
#[path = "texture_conversion_sample.rs"]
mod selection;

use selection::Sample;

const TEXTURE_SAMPLE: usize = 400;
const TAILED_SAMPLE: usize = 60;

fn successful<'a, 'archive>(
    report: &ExtractionReport,
    headers: &'a [Sample<'archive>],
) -> Vec<&'a Sample<'archive>> {
    assert!(
        report.failures().is_empty(),
        "unexpected extraction failures: {:?}",
        report.failures()
    );
    assert!(
        report.cancelled().is_empty(),
        "the conversion was not cancelled"
    );
    assert!(
        !report.archive_changed(),
        "the source archive stayed stable"
    );
    let sources: HashMap<_, _> = headers
        .iter()
        .map(|header| (header.entry.path(), header))
        .collect();
    for failure in report.unconverted() {
        assert!(
            sources
                .get(failure.path())
                .is_some_and(|header| artifacts::unsupported(
                    failure.error(),
                    header.header.format()
                )),
            "unexpected conversion failure: {failure:?}"
        );
    }
    let unsupported: HashSet<_> = report
        .unconverted()
        .iter()
        .map(p4kx_extraction::EntryFailure::path)
        .collect();
    headers
        .iter()
        .filter(|header| !unsupported.contains(header.entry.path()))
        .collect()
}

fn expected_images(
    target: &Path,
    converted: &[&Sample<'_>],
) -> artifacts::CheckResult<Vec<artifacts::ExpectedImage>> {
    let mut expected = Vec::new();
    for header in converted {
        expected.extend(artifacts::expected(
            target,
            header.entry.path(),
            &header.header,
            0,
        )?);
        let tail = target.join(format!("{}.a", header.entry.path()).replace('\\', "/"));
        assert!(
            !tail.exists(),
            "a converted alpha tail is consumed: {}",
            tail.display()
        );
    }
    Ok(expected)
}

/// Requires an image plan when a selected source has an existing decoder.
fn assert_supported_image_coverage(headers: &[Sample<'_>], expected: &[artifacts::ExpectedImage]) {
    assert!(
        !headers
            .iter()
            .any(|header| selection::supports_images(&header.header))
            || !expected.is_empty(),
        "selected supported sources require a nonempty validated image set"
    );
}

fn verify_sample(
    archive: &Archive,
    headers: &[Sample<'_>],
    entries: &[&Entry],
    target: &Path,
) -> TestResult {
    fs::create_dir_all(target)?;
    let options = ExtractOptions {
        convert: true,
        ..ExtractOptions::default()
    };
    let report = extract(archive, entries, target, &options);
    assert_eq!(
        report.extracted(),
        entries.len(),
        "every requested source entry completes: {report:?}"
    );
    let converted = successful(&report, headers);
    let expected = expected_images(target, &converted)?;
    assert_supported_image_coverage(headers, &expected);
    artifacts::check(target, &expected)?;
    artifacts::check_receipts(target, &expected, &report)?;
    writeln!(
        std::io::stderr(),
        "{} sampled headers; {} converted; {} explicitly unsupported; {} validated images",
        headers.len(),
        converted.len(),
        report.unconverted().len(),
        expected.len()
    )?;
    let before = artifacts::snapshot(target)?;
    let again = extract(
        archive,
        entries,
        target,
        &ExtractOptions {
            policy: ConflictPolicy::IfChanged,
            ..options
        },
    );
    let repeated = successful(&again, headers);
    assert!(
        repeated
            .iter()
            .map(|header| header.entry.path())
            .eq(converted.iter().map(|header| header.entry.path())),
        "the same headers convert or remain explicitly unsupported"
    );
    assert!(
        again.texture_artifacts().is_empty(),
        "no image was rewritten"
    );
    assert_eq!(
        again.skipped().len(),
        entries.len(),
        "IfChanged keeps every output: {again:?}"
    );
    artifacts::check(target, &expected)?;
    assert_eq!(
        artifacts::snapshot(target)?,
        before,
        "every expected image remains byte-identical"
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn converts_a_sample_of_real_textures_once_each() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let headers = selection::select(&archive, TEXTURE_SAMPLE, false)?;
    assert!(!headers.is_empty(), "the Archive has DDS headers");
    let entries: Vec<_> = headers.iter().map(|header| header.entry).collect();
    let target = tempfile::tempdir()?;
    verify_sample(&archive, &headers, &entries, target.path())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn converts_real_textures_with_alpha_tails_without_writing_the_tails() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let headers = selection::select(&archive, TAILED_SAMPLE, true)?;
    if headers.is_empty() {
        writeln!(
            std::io::stderr(),
            "no alpha-tail texture exists in this Archive"
        )?;
        return Ok(());
    }
    let entries = selection::members(&archive, &headers);
    let target = tempfile::tempdir()?;
    verify_sample(&archive, &headers, &entries, target.path())
}
