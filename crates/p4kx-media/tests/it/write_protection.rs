// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_media::{ExportOptions, MediaError};

use super::support::{self, TestResult};

#[test]
fn actual_source_protection_refuses_its_installation_and_allows_an_outside_generation() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let installation = folder.path().join("synthetic-installation");
    fs::create_dir(&installation)?;
    fs::create_dir(installation.join("Bin64"))?;
    let archive = support::archive(&installation)?;
    let original = fs::read(archive.path())?;
    let refused = installation.join("media");
    let error = p4kx_media::export(&archive, &refused, &ExportOptions::default())
        .err()
        .ok_or("source installation accepted as output")?;
    assert!(matches!(error, MediaError::Protected(cause) if cause.installation().is_some()));
    assert!(!refused.exists(), "no installation directory was created");
    let output = folder.path().join("outside");
    let index = support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions::default(),
    )?)?;
    assert!(index.complete);
    assert!(output.join("media-index.json").is_file());
    assert_eq!(fs::read(archive.path())?, original);
    Ok(())
}
