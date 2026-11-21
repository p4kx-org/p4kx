// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::os::windows::fs::MetadataExt as _;
use std::os::windows::process::CommandExt as _;
use std::path::Path;
use std::process::Command;

use p4kx_media::{ExportOptions, MediaError};

use super::support::{self, TestResult};

fn junction(script: &Path, path: &Path, target: &Path) -> TestResult {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    fs::write(
        script,
        "param([string]$Path, [string]$Target)\n$ErrorActionPreference = 'Stop'\nNew-Item -ItemType Junction -Path $Path -Target $Target | Out-Null\n",
    )?;
    let result = Command::new("pwsh")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["-NoLogo", "-NoProfile", "-File"])
        .arg(script)
        .arg("-Path")
        .arg(path)
        .arg("-Target")
        .arg(target)
        .output()?;
    if !result.status.success() {
        return Err(format!(
            "junction fixture failed: {}",
            String::from_utf8_lossy(&result.stderr)
        )
        .into());
    }

    assert_ne!(
        fs::symlink_metadata(path)?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT,
        0,
        "fixture is an actual Windows reparse point"
    );
    Ok(())
}

#[test]
fn an_unprotected_output_junction_is_refused_before_any_export_write() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let real = folder.path().join("ordinary-output");
    fs::create_dir(&real)?;
    fs::write(real.join("sentinel"), b"unchanged")?;
    let alias = folder.path().join("junction-output");
    junction(&folder.path().join("junction.ps1"), &alias, &real)?;
    let result = p4kx_media::export(&archive, &alias, &ExportOptions::default());
    assert!(
        matches!(result, Err(MediaError::Destination(_))),
        "ordinary junctions require an explicit plain-path refusal"
    );
    assert_eq!(fs::read(real.join("sentinel"))?, b"unchanged");
    assert_eq!(fs::read_dir(&real)?.count(), 1);
    fs::remove_dir(&alias)?;
    Ok(())
}

#[test]
fn index_and_lock_reparse_targets_are_refused_before_opening_them() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    for name in ["media-index.json", ".media.lock"] {
        let output = folder.path().join(format!("output-{name}"));
        let real = folder.path().join(format!("ordinary-{name}"));
        fs::create_dir(&output)?;
        fs::create_dir(&real)?;
        fs::write(real.join("sentinel"), b"unchanged")?;
        let alias = output.join(name);
        junction(&folder.path().join("junction.ps1"), &alias, &real)?;
        assert!(
            matches!(
                p4kx_media::export(&archive, &output, &ExportOptions::default()),
                Err(MediaError::Destination(_))
            ),
            "{name} cannot redirect an output read or write"
        );
        assert_eq!(fs::read(real.join("sentinel"))?, b"unchanged");
        assert_eq!(fs::read_dir(&real)?.count(), 1);
        assert!(
            !output.join("generations").exists(),
            "no generation was placed"
        );
        fs::remove_dir(&alias)?;
    }
    Ok(())
}

#[test]
fn an_artifact_directory_junction_cannot_hide_or_replace_the_valid_root_index() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    let index = support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions::default(),
    )?)?;
    let prior = fs::read(output.join("media-index.json"))?;
    let raw = output.join(index.artifact_base).join("raw");
    let relocated = folder.path().join("retained-original-raw");
    fs::rename(&raw, &relocated)?;
    junction(&folder.path().join("junction.ps1"), &raw, &relocated)?;
    assert!(
        matches!(
            p4kx_media::export(&archive, &output, &ExportOptions::default()),
            Err(MediaError::Destination(_))
        ),
        "artifact ancestors must reject Windows reparse points"
    );
    assert_eq!(fs::read(output.join("media-index.json"))?, prior);
    assert_eq!(fs::read_dir(&relocated)?.count(), 3);
    fs::remove_dir(&raw)?;
    Ok(())
}
