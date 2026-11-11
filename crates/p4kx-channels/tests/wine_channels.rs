// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Synthetic Wine-style Channel discovery and path overrides.

use std::error::Error;
use std::fs;
use std::io::Read as _;
use std::path::Path;

use p4kx_archive::Archive;
use p4kx_channels::{DiscoveryLimits, SearchPaths, SearchRecipe, WineEnvironment};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key, write_build_manifest};

type TestResult = Result<(), Box<dyn Error>>;

const PAYLOAD: &[u8] = b"synthetic Wine Archive Entry";

#[test]
fn a_mixed_case_wine_profile_yields_the_real_channel_and_same_scan_patch() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let compat = temporary.path().join("compat");
    let prefix = compat.join("pfx");
    let library = prefix.join("DrIvE_C/GaMeS/StArCiTiZeN");
    let channel = library.join("PTU");
    let logs = prefix.join("DrIvE_C/UsErS/Tester/AppDaTa/RoAmInG/RSILauncher/LoGs");
    fs::create_dir_all(&channel)?;
    fs::create_dir_all(&logs)?;
    write_archive(&channel.join("Data.p4k"))?;
    write_build_manifest(
        &channel,
        "sc-alpha-5.0.0",
        "5.0.0.0",
        "42",
        "synthetic-build",
    )?;
    let line = concat!(
        "{ \"message\": \"Installing Star Citizen PTU at C:\\\\Games\\\\StarCitizen (force DP false)\" }\n",
        "(SC PTU 5.0.0-ptu.42)\n",
    );
    fs::write(logs.join("log.log"), line)?;
    let recipe = SearchRecipe::with_environment(
        SearchPaths::default(),
        WineEnvironment::new(None, None).with_compat_data(Some(compat.into_os_string())),
        DiscoveryLimits::default(),
    );
    let snapshot = recipe.snapshot();
    let report = snapshot.detect();
    assert!(
        report.is_complete(),
        "all unique case fallbacks are complete: {:?}",
        report.issues()
    );
    assert_eq!(
        report.value().channels().len(),
        1,
        "one actual library supplies the Channel"
    );
    let found = report.value().channels().first().ok_or("detected PTU")?;
    assert_archive_path(
        found.archive(),
        &channel.join("Data.p4k"),
        &prefix.join("drive_c/Games/StarCitizen/PTU/Data.p4k"),
    )?;
    assert_archive_payload(found.archive())?;
    assert_eq!(
        found.build().ok_or("PTU build")?.public_patch(),
        Some("5.0.0"),
        "the same source scan attaches the patch by exact change number"
    );
    assert_eq!(
        report.value().patches().get("42"),
        Some("5.0.0"),
        "all consumers receive the same map"
    );
    Ok(())
}

fn write_archive(path: &Path) -> TestResult {
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\fixture.bin", PAYLOAD.to_vec()).encrypted())
        .write_to(path)?;
    Ok(())
}

fn assert_archive_payload(path: &Path) -> TestResult {
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let entry = archive.entries().first().ok_or("synthetic Archive Entry")?;
    assert!(entry.is_encrypted(), "the fixture exercises key use");
    let mut payload = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut payload)?;
    assert_eq!(payload, PAYLOAD, "the detected Archive can be decoded");
    Ok(())
}

fn assert_archive_path(actual: &Path, original: &Path, requested: &Path) -> TestResult {
    assert!(
        same_file::is_same_file(actual, original)?,
        "the detected Archive reaches the original synthetic file"
    );
    if !fs::exists(requested)? {
        assert_eq!(
            actual, original,
            "a case-sensitive fallback retains the discovered host spelling"
        );
    }
    Ok(())
}
