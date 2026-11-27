// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{Archive, ConflictPolicy};
use p4kx_media::ExportOptions;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::support::{self, TestResult};

#[test]
fn an_existing_readonly_empty_lock_supports_successive_exports_without_content_changes()
-> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    fs::create_dir(&output)?;
    let lock = output.join(".media.lock");
    fs::write(&lock, [])?;
    let original = fs::metadata(&lock)?.permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&lock, readonly)?;
    let result = (|| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        p4kx_media::export(&archive, &output, &ExportOptions::default())?;
        p4kx_media::export(&archive, &output, &ExportOptions::default())?;
        Ok(fs::read(&lock)?)
    })();
    fs::set_permissions(&lock, original)?;
    assert_eq!(result?, b"");
    Ok(())
}

#[test]
fn nonempty_foreign_and_source_hardlink_locks_are_refused_without_mutation() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let source = fs::read(archive.path())?;
    for hardlink in [false, true] {
        let output = folder
            .path()
            .join(if hardlink { "linked" } else { "foreign" });
        fs::create_dir(&output)?;
        let lock = output.join(".media.lock");
        if hardlink {
            fs::hard_link(archive.path(), &lock)?;
        } else {
            fs::write(&lock, b"foreign lock contents")?;
        }
        let original = fs::read(&lock)?;
        assert!(p4kx_media::export(&archive, &output, &ExportOptions::default()).is_err());
        assert_eq!(fs::read(&lock)?, original);
        assert!(!output.join("media-index.json").exists());
    }
    assert_eq!(fs::read(archive.path())?, source);
    Ok(())
}

#[test]
fn a_new_lock_survives_index_validation_failure_and_remains_reusable() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    fs::create_dir(&output)?;
    let index = output.join("media-index.json");
    fs::write(&index, b"foreign malformed index")?;
    assert!(p4kx_media::export(&archive, &output, &ExportOptions::default()).is_err());
    assert_eq!(fs::read(&index)?, b"foreign malformed index");
    assert_eq!(fs::read(output.join(".media.lock"))?, b"");
    fs::remove_file(&index)?;
    support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions::default(),
    )?)?;
    assert_eq!(fs::read(output.join(".media.lock"))?, b"");
    Ok(())
}

#[test]
fn replacing_the_top_index_preserves_unrelated_output_files_and_the_previous_generation()
-> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    let first = support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions::default(),
    )?)?;
    let foreign = output.join("precious.txt");
    fs::write(&foreign, b"foreign bytes")?;
    let next_path = folder.path().join("next.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("next.bin", b"next bytes"))
        .write_to(&next_path)?;
    let next = Archive::open(next_path)?;
    let options = ExportOptions {
        conflict: ConflictPolicy::Overwrite,
        ..ExportOptions::default()
    };
    let second = support::published(p4kx_media::export(&next, &output, &options)?)?;
    assert_ne!(second.generation, first.generation);
    assert_eq!(fs::read(&foreign)?, b"foreign bytes");
    assert!(output.join(first.artifact_base).is_dir());
    assert_eq!(
        serde_json::from_slice::<p4kx_media::Index>(&fs::read(output.join("media-index.json"))?)?
            .generation,
        second.generation
    );
    Ok(())
}
