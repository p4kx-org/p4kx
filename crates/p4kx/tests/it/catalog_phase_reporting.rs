// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_testkit::{TestContainer, TestEntry};

use super::catalog_support::{EXPORTED_AT, english, german, write_archive_with};
use super::{TestResult, p4kx};

const CATALOG_PHASES: &[&str] = &[
    "read-dataforge",
    "read-inputs",
    "open-dataforge",
    "read-localization",
    "index-localization",
    "initialize-origins",
    "crafting",
    "world-placements",
    "records",
    "projections",
    "resolve-loadouts",
    "project-locations",
    "project-missions",
    "read-mission-dependencies",
    "project-mission-definitions",
    "project-contract-templates",
    "project-mission-broker-entries",
    "project-mission-location-templates",
    "project-contract-generators",
    "link-mission-factions",
    "project-configurations",
    "project-shops",
    "resolve-jump-operators",
    "read-independent-configurations",
    "project-particle-libraries",
    "project-weapon-animation",
    "finalize-projections",
    "export-health",
    "provenance",
    "measure-json",
    "write-json",
];

#[test]
fn phases_are_logged_with_redirected_stderr_without_changing_the_catalog() -> TestResult {
    let dir = tempfile::tempdir()?;
    let container = TestContainer::new()
        .entry(TestEntry::stored(
            "syntheticsystem.xml",
            b"<ObjectContainer><ChildObjectContainers/></ObjectContainer>".as_slice(),
        ))
        .build();
    let system = TestEntry::stored(
        r"Data\ObjectContainers\PU\system\synthetic\syntheticsystem.socpak",
        container,
    );
    let archive = write_archive_with(dir.path(), &[english(), german(), system])?;
    let out = dir.path().join("p4kx-catalog.json.zst");
    let run = |flags: &[&str]| -> Result<_, Box<dyn std::error::Error>> {
        Ok(p4kx(&["catalog", "--json"])?
            .arg(&archive)
            .arg("--out")
            .arg(&out)
            .args(["--exported-at", EXPORTED_AT])
            .args(flags)
            .output()?)
    };
    let standard = run(&[])?;
    assert_eq!(
        standard.status.code(),
        Some(0),
        "normal output: {standard:?}"
    );
    let normal_stderr = std::str::from_utf8(&standard.stderr)?;
    assert!(
        !normal_stderr.contains("catalog phase: ") && !normal_stderr.contains("catalog storage: "),
        "phase reporting is opt-in"
    );
    let normal_bytes = fs::read(&out)?;
    let traced = run(&["--progress-phases"])?;
    assert_eq!(traced.status.code(), Some(0), "traced output: {traced:?}");
    assert_eq!(traced.stdout, standard.stdout, "unchanged JSON summary");
    assert!(
        fs::read(&out)? == normal_bytes,
        "unchanged compressed export"
    );
    let traced_stderr = std::str::from_utf8(&traced.stderr)?;
    let phases: Vec<_> = traced_stderr
        .lines()
        .filter_map(|line| line.strip_prefix("catalog phase: "))
        .collect();
    assert_eq!(
        phases, CATALOG_PHASES,
        "ordered preparation and write phases"
    );
    check_storage(traced_stderr, fs::metadata(out)?.len())?;
    Ok(())
}

fn check_storage(stderr: &str, published_bytes: u64) -> TestResult {
    let mut reports = stderr
        .lines()
        .filter_map(|line| line.strip_prefix("catalog storage: "));
    let mut fields = reports
        .next()
        .ok_or("missing measured temporary storage")?
        .split_whitespace();
    let mut bytes = |key| -> Result<u128, Box<dyn std::error::Error>> {
        Ok(fields
            .next()
            .and_then(|field| field.strip_prefix(key))
            .ok_or("missing storage byte count")?
            .parse()?)
    };
    let staged = bytes("staged_row_bytes=")?;
    let publication = bytes("publication_bytes=")?;
    let peak = bytes("peak_temporary_bytes=")?;
    assert!(reports.next().is_none(), "one report after publication");
    assert!(fields.next().is_none(), "only the three byte counts");
    assert!(staged > 0, "the observed world root was staged on disk");
    assert_eq!(
        publication,
        u128::from(published_bytes),
        "the reported publication size equals the finished file"
    );
    assert_eq!(peak, staged + publication, "the temporary files overlap");
    Ok(())
}
