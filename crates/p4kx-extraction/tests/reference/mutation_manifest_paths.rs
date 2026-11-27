// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Untrusted manifest paths and bounds are checked before artifact reads.

use p4kx_testkit::TestEntry;
use serde_json::json;

use super::TestResult;
use super::manifest_fixtures::{Fixture, freeze};
use super::reference_error::Code;
use super::reference_manifest::{
    ArtifactRole, MANIFEST_NAME, MAX_ARTIFACT_BYTES, MAX_MANIFEST_BYTES,
};
use super::{reference_load, unp4k};

fn fixture() -> Result<(Fixture, super::reference_manifest::Manifest), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(vec![TestEntry::stored("Data/a.bin", b"data".to_vec())])?;
    let manifest = fixture.raw(b"data")?;
    Ok((fixture, manifest))
}

#[test]
fn parent_absolute_drive_and_ambiguous_paths_are_rejected_before_opening() -> TestResult {
    let (fixture, original) = fixture()?;
    for path in [
        "../outside",
        "/outside",
        "C:/outside",
        "\\\\server\\share\\file",
        "a/../../outside",
        "a//file",
        "a/./file",
        "a:file",
    ] {
        let mut manifest = original.clone();
        let payload = manifest
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.role == ArtifactRole::Payload)
            .ok_or("payload missing")?;
        payload.path = path.to_owned();
        manifest
            .expected
            .first_mut()
            .and_then(|source| source.targets.first_mut())
            .ok_or("target missing")?
            .path = path.to_owned();
        freeze(&mut manifest)?;
        fixture.save(&manifest)?;
        let error = unp4k::verify(&fixture.archive, &fixture.root)
            .err()
            .ok_or("unsafe path passed")?;
        assert_eq!(
            error.code,
            Code::Path,
            "unsafe paths fail before artifact access"
        );
    }
    Ok(())
}

#[test]
fn case_aliases_and_oversized_artifact_claims_fail_without_allocating_payloads() -> TestResult {
    let (fixture, original) = fixture()?;
    let mut alias = original.clone();
    let mut duplicate = alias
        .artifacts
        .iter()
        .find(|artifact| artifact.role == ArtifactRole::Payload)
        .ok_or("payload missing")?
        .clone();
    duplicate.path = duplicate.path.to_uppercase();
    alias.artifacts.push(duplicate);
    freeze(&mut alias)?;
    fixture.save(&alias)?;
    let error = unp4k::verify(&fixture.archive, &fixture.root)
        .err()
        .ok_or("case collision passed")?;
    assert_eq!(
        error.code,
        Code::Path,
        "case aliases collide on all platforms"
    );
    let mut oversized = original;
    oversized
        .artifacts
        .first_mut()
        .and_then(|artifact| artifact.fingerprint.as_mut())
        .ok_or("fingerprint missing")?
        .bytes = MAX_ARTIFACT_BYTES + 1;
    fixture.save(&oversized)?;
    let error = unp4k::verify(&fixture.archive, &fixture.root)
        .err()
        .ok_or("oversized claim passed")?;
    assert_eq!(
        error.code,
        Code::Bounds,
        "declared payload bounds precede reads and allocations"
    );
    Ok(())
}

#[test]
fn malformed_unknown_and_missing_manifest_contracts_are_rejected() -> TestResult {
    let (fixture, manifest) = fixture()?;
    for mutation in 0..3 {
        let mut value = serde_json::to_value(&manifest)?;
        match mutation {
            0 => value["schemaVersion"] = json!(999),
            1 => value["source"]["archive"]["digest"]["algorithm"] = json!("sha512"),
            _ => {
                value["producer"]
                    .as_object_mut()
                    .ok_or("producer not an object")?
                    .remove("binary");
            }
        }
        fixture.write(MANIFEST_NAME, &serde_json::to_vec(&value)?)?;
        assert!(
            unp4k::verify(&fixture.archive, &fixture.root).is_err(),
            "unknown contracts and missing fingerprints cannot establish trust"
        );
    }
    fixture.write(MANIFEST_NAME, b"{ malformed")?;
    let error = reference_load::read(&fixture.root)
        .err()
        .ok_or("malformed JSON passed")?;
    assert_eq!(
        error.code,
        Code::Manifest,
        "malformed JSON is a manifest error"
    );
    Ok(())
}

#[test]
fn an_oversized_sparse_manifest_is_rejected_before_reading_its_contents() -> TestResult {
    let (fixture, _) = fixture()?;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(fixture.root.join(MANIFEST_NAME))?;
    file.set_len(MAX_MANIFEST_BYTES + 1)?;
    drop(file);
    let error = reference_load::read(&fixture.root)
        .err()
        .ok_or("oversized manifest passed")?;
    assert_eq!(
        error.code,
        Code::Bounds,
        "metadata bound is checked before allocation"
    );
    Ok(())
}

#[cfg(windows)]
fn junction_escape(root: &std::path::Path, target: &std::path::Path) -> TestResult {
    let raw = root.join("raw");
    std::fs::remove_dir(&raw)?;
    let output = std::process::Command::new("pwsh")
        .args([
            "-NoProfile", "-NonInteractive", "-Command",
            "$ErrorActionPreference = 'Stop'; New-Item -ItemType Junction -Path $env:P4KX_TEST_JUNCTION -Target $env:P4KX_TEST_TARGET | Out-Null",
        ])
        .env("P4KX_TEST_JUNCTION", &raw)
        .env("P4KX_TEST_TARGET", target)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "native junction creation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(())
}

#[test]
fn linked_payloads_never_follow_a_resolved_escape() -> TestResult {
    let (fixture, _) = fixture()?;
    let outside = fixture
        .root
        .parent()
        .ok_or("reference parent missing")?
        .join("outside");
    std::fs::create_dir(&outside)?;
    let target = outside.join("a.bin");
    std::fs::write(&target, b"data")?;
    let path = fixture.root.join("raw/a.bin");
    std::fs::remove_file(&path)?;
    #[cfg(windows)]
    match std::os::windows::fs::symlink_file(&target, &path) {
        Ok(()) => {}
        Err(error) => {
            const PRIVILEGE_NOT_HELD: i32 = 1314;
            if error.raw_os_error() != Some(PRIVILEGE_NOT_HELD)
                && error.kind() != std::io::ErrorKind::PermissionDenied
            {
                return Err(error.into());
            }
            junction_escape(&fixture.root, &outside)?;
        }
    }
    #[cfg(not(windows))]
    std::os::unix::fs::symlink(&target, &path)?;
    let result = unp4k::verify(&fixture.archive, &fixture.root);
    #[cfg(windows)]
    if fixture.root.join("raw").is_symlink() {
        std::fs::remove_dir(fixture.root.join("raw"))?;
    }
    let error = result.err().ok_or("linked payload passed")?;
    assert_eq!(
        error.code,
        Code::Path,
        "linked escapes are refused before payload opening"
    );
    Ok(())
}
