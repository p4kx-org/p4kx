// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use p4kx_channels::{
    ChannelSource, DiscoveryIssueKind, DiscoveryLimits, SearchPaths, SearchRecipe, WineEnvironment,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry, write_build_manifest};

use super::reported_to;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

struct Fixture {
    _temporary: tempfile::TempDir,
    native: PathBuf,
    steam: PathBuf,
    archive: PathBuf,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let temporary = tempfile::tempdir()?;
        let native = temporary.path().join("native");
        let steam = temporary.path().join("steam");
        let channel = temporary.path().join("manual/LIVE");
        fs::create_dir_all(native.join("logs"))?;
        fs::create_dir_all(steam.join("steamapps"))?;
        fs::create_dir_all(&channel)?;
        fs::write(native.join("logs/log.log"), "(SC LIVE 5.0.0-live.42)\n")?;
        fs::write(
            steam.join("steamapps/libraryfolders.vdf"),
            "\"libraryfolders\" {",
        )?;
        let archive = channel.join("Data.p4k");
        ArchiveBuilder::new()
            .entry(TestEntry::stored("readme.txt", b"synthetic".to_vec()))
            .write_to(&archive)?;
        write_build_manifest(
            &channel,
            "sc-alpha-5.0.0",
            "5.0.1.1234",
            "42",
            "00000000-0000-4000-8000-000000000042",
        )?;
        Ok(Self {
            _temporary: temporary,
            native,
            steam,
            archive,
        })
    }

    fn recipe(&self) -> SearchRecipe {
        let paths = SearchPaths {
            launcher: Some(self.native.clone()),
            manual_archives: vec![self.archive.clone()],
            ..SearchPaths::default()
        };
        SearchRecipe::with_environment(
            paths,
            WineEnvironment::new(None, None).with_steam_roots(vec![self.steam.clone()]),
            DiscoveryLimits::default(),
        )
    }

    fn config(&self) -> PathBuf {
        self.steam.join("steamapps/libraryfolders.vdf")
    }
}

#[test]
fn a_partial_patch_report_keeps_good_data_and_delivers_its_typed_source_reason() -> TestResult {
    let fixture = Fixture::new()?;
    let report = fixture.recipe().snapshot().read_patches();
    assert!(
        !report.is_complete(),
        "the actual malformed config produces a partial report"
    );
    let issue = report
        .issues()
        .iter()
        .find(|issue| matches!(issue.kind(), DiscoveryIssueKind::MalformedConfig))
        .ok_or("the malformed configuration issue is missing")?;
    assert_eq!(
        issue.path(),
        fixture.config(),
        "the actual malformed source path survives until the frontend boundary"
    );
    let source_path = issue.path().display().to_string();
    let mut notices = Vec::new();
    let patches = reported_to(report, &mut notices)?;
    assert_eq!(
        patches.get("42"),
        Some("5.0.0"),
        "a good log remains usable after discovery fails"
    );
    let text = String::from_utf8(notices)?;
    assert!(
        text.contains(&source_path),
        "the warning names the actual affected source: {text}"
    );
    assert!(
        text.contains("malformed Steam library configuration"),
        "the warning preserves the technical reason: {text}"
    );
    Ok(())
}

#[test]
fn a_partial_detection_report_preserves_the_manual_channel_and_same_scan_patch() -> TestResult {
    let fixture = Fixture::new()?;
    let report = fixture.recipe().snapshot().detect();
    let mut notices = Vec::new();
    let detected = reported_to(report, &mut notices)?;
    let (channels, patches) = detected.into_parts();
    assert_eq!(
        channels.len(),
        1,
        "automatic failure does not discard the manual fallback"
    );
    let channel = channels
        .first()
        .ok_or_else(|| io::Error::other("missing synthetic manual Channel"))?;
    assert_eq!(
        channel.archive(),
        fixture.archive.as_path(),
        "the actual manual Archive remains selected"
    );
    assert_eq!(
        channel.source(),
        ChannelSource::Manual,
        "discovery keeps the manual source meaning"
    );
    assert_eq!(
        channel.build().and_then(|build| build.public_patch()),
        Some("5.0.0"),
        "the Channel receives the same measured patch as the returned map"
    );
    assert_eq!(
        patches.get("42"),
        Some("5.0.0"),
        "the measured patch part remains unchanged"
    );
    assert!(
        !notices.is_empty(),
        "usable partial Channels still report the incomplete source"
    );
    Ok(())
}

struct FailedDelivery;

impl Write for FailedDelivery {
    fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "synthetic warning delivery failure",
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn failed_warning_delivery_is_returned_before_releasing_partial_data() -> TestResult {
    let fixture = Fixture::new()?;
    let report = fixture.recipe().snapshot().read_patches();
    let result = reported_to(report, &mut FailedDelivery);
    let error = result
        .err()
        .ok_or_else(|| io::Error::other("warning failure was swallowed"))?;
    assert_eq!(
        error.kind(),
        io::ErrorKind::BrokenPipe,
        "the original output error remains typed"
    );
    assert!(
        error
            .to_string()
            .contains("synthetic warning delivery failure"),
        "the original delivery reason remains available"
    );
    Ok(())
}

#[test]
fn complete_discovery_needs_no_warning_write_and_keeps_the_patch() -> TestResult {
    let fixture = Fixture::new()?;
    fs::remove_file(fixture.config())?;
    let report = fixture.recipe().snapshot().read_patches();
    assert!(
        report.is_complete(),
        "an initially absent config is ordinary absence: {:?}",
        report.issues()
    );
    let patches = reported_to(report, &mut FailedDelivery)?;
    assert_eq!(
        patches.get("42"),
        Some("5.0.0"),
        "a warning-free result needs no delivery"
    );
    Ok(())
}
