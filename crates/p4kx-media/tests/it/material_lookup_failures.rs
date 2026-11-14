// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{Archive, OpenOptions, archive_error_in};
use p4kx_media::{MaterialProvider, MediaError, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::json;

use super::material_lookup_support::{field, reference, status, target_index};
use super::material_support::{self as support, TestResult};

const SOURCE: &[u8] = br#"<Material><MatLayers><Layer Path="b"/></MatLayers></Material>"#;

#[test]
fn an_unreadable_structured_target_keeps_its_failed_row_separate_from_identity() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(
        folder.path(),
        [
            TestEntry::stored("Data/a.mtl", SOURCE),
            TestEntry::stored("Data/b.mtl", b"<Material"),
        ],
    )?;
    let output = folder.path().join("out");
    let registry = support::registry(MaterialProvider::default())?;
    let failure = p4kx_media::export_with(&archive, &output, &support::options(), &registry)
        .err()
        .ok_or("unreadable target published as complete")?;
    assert!(
        support::projection_loss(&failure),
        "target readability is independent of its physical existence"
    );
    assert!(
        !output.join("media-index.json").exists(),
        "target readability is independent of its physical existence"
    );
    let mut options = support::options();
    options.diagnostic = true;
    let records = support::diagnostic_records(p4kx_media::export_with(
        &archive, &output, &options, &registry,
    )?)?;
    verify_failed_target(&records)?;
    Ok(())
}

#[test]
fn an_unreadable_target_entry_keeps_the_real_archive_error_and_physical_source() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/a.mtl", SOURCE))
        .entry(TestEntry::zstd("Data/b.mtl", b"<Material/>").encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, None)?;
    let output = folder.path().join("out");
    let failure = p4kx_media::export_with(
        &archive,
        &output,
        &support::options(),
        &support::registry(MaterialProvider::default())?,
    )
    .err()
    .ok_or("missing target key was ignored")?;
    assert!(
        archive_error_in(&failure)
            .ok_or("original archive cause")?
            .is_key_required(),
        "an unreadable target entry keeps the real archive error and physical source"
    );
    let MediaError::Source { entry, .. } = failure else {
        return Err("source context".into());
    };
    let p4kx_media::Identity::Entry { entry, .. } = entry.identity else {
        return Err("physical entry".into());
    };
    assert_eq!(
        entry.entry_index, 1,
        "unreadability is reported at the actual target"
    );
    assert!(
        !output.join("media-index.json").exists(),
        "an unreadable target entry keeps the real archive error and physical source"
    );
    Ok(())
}

#[test]
fn a_recovered_directory_does_not_turn_an_unseen_target_into_missing() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("recovered.p4k");
    let mut bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/a.mtl", SOURCE))
        .build();
    bytes.truncate(bytes.len().checked_sub(22).ok_or("end record")?);
    fs::write(&path, bytes)?;
    let archive = Archive::open_with(&path, OpenOptions::new().recover(true))?;
    assert!(
        archive
            .issues()
            .iter()
            .any(|(_, issue)| issue.is_archive_wide()),
        "a recovered directory does not turn an unseen target into missing"
    );
    let mut options = support::options();
    options.diagnostic = true;
    let records = support::diagnostic_records(p4kx_media::export_with(
        &archive,
        &folder.path().join("out"),
        &options,
        &support::registry(MaterialProvider::default())?,
    )?)?;
    let reference = reference(records.first().ok_or("recovered source")?, 0)?;
    status(reference, "unavailable")?;
    assert_eq!(
        field(reference, "/resolution/reason/type")?,
        &json!("incomplete-inventory"),
        "a recovered directory does not turn an unseen target into missing"
    );
    assert_eq!(
        field(reference, "/candidates_complete")?,
        &json!(false),
        "a recovered directory does not turn an unseen target into missing"
    );
    Ok(())
}

fn verify_failed_target(records: &[MediaRecord]) -> TestResult {
    assert_eq!(
        records.len(),
        2,
        "target readability is independent of its physical existence"
    );
    let source = records.first().ok_or("source")?;
    let target = records.get(1).ok_or("failed target")?;
    status(reference(source, 0)?, "resolved")?;
    assert_eq!(
        target_index(reference(source, 0)?)?,
        1,
        "target readability is independent of its physical existence"
    );
    assert_eq!(
        target.status,
        Status::Failed,
        "target readability is independent of its physical existence"
    );
    assert_eq!(
        target.subresource.namespace, "material-library-failure",
        "target readability is independent of its physical existence"
    );
    assert_eq!(
        field(&target.data, "/payload/reason/type")?,
        &json!("xml"),
        "target readability is independent of its physical existence"
    );
    assert_eq!(
        target
            .source
            .raw
            .as_ref()
            .ok_or("retained failed input")?
            .size,
        9,
        "target readability is independent of its physical existence"
    );
    Ok(())
}
