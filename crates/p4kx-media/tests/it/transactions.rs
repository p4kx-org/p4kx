// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::ConflictPolicy;
use p4kx_media::{ExportOptions, ExportOutcome};

use super::support::{self, TestResult};

#[test]
fn skip_and_if_changed_retain_the_validated_complete_generation() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    let first = support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions::default(),
    )?)?;
    let bytes = fs::read(output.join("media-index.json"))?;
    assert!(matches!(
        p4kx_media::export(&archive, &output, &ExportOptions::default())?,
        ExportOutcome::Skipped { .. }
    ));
    let options = ExportOptions {
        conflict: ConflictPolicy::IfChanged,
        ..ExportOptions::default()
    };
    let result = p4kx_media::export(&archive, &output, &options)?;
    let ExportOutcome::Published {
        index,
        changed: false,
    } = result
    else {
        return Err("content generation identity is deterministic".into());
    };
    assert_eq!(index.artifact_base, first.artifact_base);
    assert_eq!(support::records(&output, &index, "archive.jsonl")?.len(), 4);
    assert_eq!(fs::read(output.join("media-index.json"))?, bytes);
    assert!(output.join("generations").join(first.generation).is_dir());
    Ok(())
}

#[test]
fn a_damaged_prior_artifact_is_refused_even_by_skip_policy() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    let index = support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions::default(),
    )?)?;
    let kind = index.kinds.first().ok_or("kind")?;
    fs::write(
        output.join(index.artifact_base).join(&kind.artifact.path),
        b"corrupt",
    )?;
    assert!(
        p4kx_media::export(&archive, &output, &ExportOptions::default()).is_err(),
        "existing manifest is not enough without actual artifact integrity"
    );
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(output.join(".media.lock"))?;
    lock.try_lock()?;
    lock.unlock()?;
    Ok(())
}
