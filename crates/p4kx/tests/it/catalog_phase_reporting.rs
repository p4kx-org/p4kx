// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use super::{TestResult, p4kx, write_archive};

#[test]
fn phases_are_logged_with_redirected_stderr_without_changing_the_catalog() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("p4kx-catalog.json.zst");
    let run = |flags: &[&str]| -> Result<_, Box<dyn std::error::Error>> {
        Ok(p4kx(&["catalog", "--json"])?
            .arg(&archive)
            .arg("--out")
            .arg(&out)
            .args(flags)
            .output()?)
    };
    let standard = run(&[])?;
    assert_eq!(
        standard.status.code(),
        Some(0),
        "normal output: {standard:?}"
    );
    assert!(
        !std::str::from_utf8(&standard.stderr)?.contains("catalog phase: "),
        "phase reporting is opt-in"
    );
    let normal_bytes = fs::read(&out)?;
    let traced = run(&["--progress-phases"])?;
    assert_eq!(traced.status.code(), Some(0), "traced output: {traced:?}");
    assert_eq!(traced.stdout, standard.stdout, "unchanged JSON summary");
    assert!(
        fs::read(out)? == normal_bytes,
        "unchanged compressed export"
    );
    let phases: Vec<_> = std::str::from_utf8(&traced.stderr)?
        .lines()
        .filter_map(|line| line.strip_prefix("catalog phase: "))
        .collect();
    assert_eq!(
        phases,
        [
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
            "project-contract-generators",
            "link-mission-factions",
            "project-configurations",
            "finalize-projections",
            "export-health",
            "provenance",
            "measure-json",
            "write-json",
        ],
        "ordered preparation and write phases"
    );
    Ok(())
}
