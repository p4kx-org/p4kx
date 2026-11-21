// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{ConflictPolicy, Filter};
use p4kx_media::{MaterialProvider, MediaError};
use p4kx_testkit::{TestContainer, TestEntry};
use serde_json::json;

use super::material_lookup_support::{field, reference, status};
use super::material_support::{self as support, TestResult};

const SOURCE: &[u8] = br#"<Material><MatLayers><Layer Path="b"/></MatLayers></Material>"#;

#[test]
fn too_many_physical_targets_are_unavailable_instead_of_first_match() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(
        folder.path(),
        [
            TestEntry::stored("Data/a.mtl", SOURCE),
            TestEntry::stored("Data/b.mtl", b"<Material/>"),
            TestEntry::stored("Data/b.mtl", b"<Material/>"),
            TestEntry::stored("Data/b.mtl", b"<Material/>"),
        ],
    )?;
    let mut options = support::options();
    options.limits.reference_candidates = 2;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &options,
        &support::registry(MaterialProvider::default())?,
    )?)?;
    let records = support::records(&output, &index)?;
    let reference = reference(records.first().ok_or("source")?, 0)?;
    status(reference, "unavailable")?;
    assert_eq!(
        field(reference, "/resolution/reason/type")?,
        &json!("candidate-limit"),
        "too many physical targets are unavailable instead of first match"
    );
    assert_eq!(
        field(reference, "/observed_candidates")?,
        &json!(3),
        "too many physical targets are unavailable instead of first match"
    );
    assert_eq!(
        field(reference, "/candidates_complete")?,
        &json!(false),
        "too many physical targets are unavailable instead of first match"
    );
    assert_eq!(
        field(reference, "/resolution/known_candidates")?
            .as_array()
            .ok_or("known")?
            .len(),
        2,
        "too many physical targets are unavailable instead of first match"
    );
    Ok(())
}

#[test]
fn index_work_and_source_admission_preserve_the_previous_committed_generation() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(
        folder.path(),
        [
            TestEntry::stored("Data/a.mtl", SOURCE),
            TestEntry::stored("Data/b.mtl", b"<Material/>"),
        ],
    )?;
    let output = folder.path().join("out");
    let registry = support::registry(MaterialProvider::default())?;
    p4kx_media::export_with(&archive, &output, &support::options(), &registry)?;
    let previous = fs::read(output.join("media-index.json"))?;
    for budget in ["metadata_work", "sources"] {
        let mut options = support::options();
        options.conflict = ConflictPolicy::Overwrite;
        if budget == "sources" {
            options.limits.sources = 1;
        } else {
            options.limits.metadata_work = 1;
        }
        let failure = p4kx_media::export_with(&archive, &output, &options, &registry)
            .err()
            .ok_or("incomplete lookup published")?;
        let MediaError::Source { cause, .. } = failure else {
            return Err("source context".into());
        };
        assert!(
            matches!(*cause, MediaError::Limit { budget: actual, limit: 1 } if actual == budget),
            "index work and source admission preserve the previous committed generation"
        );
        assert_eq!(
            fs::read(output.join("media-index.json"))?,
            previous,
            "index work and source admission preserve the previous committed generation"
        );
    }
    Ok(())
}

#[test]
fn lossy_target_display_paths_do_not_prove_a_unicode_source_token_match() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source =
        "<Material><MatLayers><Layer Path=\"bad\u{fffd}.mtl\"/></MatLayers></Material>".as_bytes();
    let archive = support::archive(
        folder.path(),
        [
            TestEntry::stored("Data/a.mtl", source),
            TestEntry::stored("unused.mtl", b"<Material/>").with_path_bytes(b"Data/bad\xff.mtl"),
        ],
    )?;
    let target = archive.entries().get(1).ok_or("lossy target entry")?;
    assert_eq!(
        target.path(),
        "Data/bad\u{fffd}.mtl",
        "the reference token uses the actual reader display path"
    );
    assert_eq!(
        archive.raw_path(target.index()),
        Some(b"Data/bad\xff.mtl".as_slice()),
        "the display coincidence still has distinct original path bytes"
    );
    for (ordinal, include) in [None, Some(Filter::parse("*a.mtl")?)]
        .into_iter()
        .enumerate()
    {
        let output = folder.path().join(format!("out-{ordinal}"));
        let mut options = support::options();
        options.include = include;
        let index = support::published(p4kx_media::export_with(
            &archive,
            &output,
            &options,
            &support::registry(MaterialProvider::default())?,
        )?)?;
        let records = support::records(&output, &index)?;
        let reference = reference(records.first().ok_or("source")?, 0)?;
        status(reference, "unavailable")?;
        assert_eq!(
            field(reference, "/resolution/reason/type")?,
            &json!("unsupported-layout"),
            "lossy target display paths do not prove a unicode source token match"
        );
        assert_eq!(
            field(
                reference,
                "/resolution/known_candidates/0/target/source/entry/raw_path_hex"
            )?,
            &json!("446174612f626164ff2e6d746c"),
            "lossy target display paths do not prove a unicode source token match"
        );
    }
    Ok(())
}

#[test]
fn a_library_without_file_references_does_not_build_an_unneeded_index() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(
        folder.path(),
        [TestEntry::stored("Data/a.mtl", b"<Material/>")],
    )?;
    let output = folder.path().join("out");
    let mut options = support::options();
    options.limits.metadata_work = 1;
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &options,
        &support::registry(MaterialProvider::default())?,
    )?)?;
    assert_eq!(
        support::records(&output, &index)?.len(),
        1,
        "source projection without file references needs no directory index"
    );
    Ok(())
}

#[test]
fn a_late_parent_lookup_admits_against_work_already_spent_in_its_nested_source() -> TestResult {
    let folder = tempfile::tempdir()?;
    let nested = TestContainer::new()
        .entry(TestEntry::stored("one.bin", b"1"))
        .entry(TestEntry::stored("two.bin", b"2"));
    let archive = support::archive(
        folder.path(),
        [
            TestEntry::stored("nested.pak", nested.build()),
            TestEntry::stored("Data/a.mtl", SOURCE),
            TestEntry::stored("Data/b.mtl", b"<Material/>"),
        ],
    )?;
    let output = folder.path().join("out");
    let registry = support::registry(MaterialProvider::default())?;
    p4kx_media::export_with(&archive, &output, &support::options(), &registry)?;
    let previous = fs::read(output.join("media-index.json"))?;
    let mut options = support::options();
    options.conflict = ConflictPolicy::Overwrite;
    options.limits.sources = 4;
    let failure = p4kx_media::export_with(&archive, &output, &options, &registry)
        .err()
        .ok_or("late lookup ignored nested traversal work")?;
    let MediaError::Source { entry, cause } = failure else {
        return Err("lookup must fail before a later physical Entry exhausts traversal".into());
    };
    let p4kx_media::Identity::Entry { entry, .. } = entry.identity else {
        return Err("lookup source identity".into());
    };
    assert_eq!(
        entry.entry_index, 1,
        "the first parent lookup uses fresh remaining admission"
    );
    assert!(
        matches!(
            *cause,
            MediaError::Limit {
                budget: "sources",
                limit: 4
            }
        ),
        "already examined nested Entries reduce parent lookup admission"
    );
    assert_eq!(
        fs::read(output.join("media-index.json"))?,
        previous,
        "late lookup admission cannot replace the committed generation"
    );
    Ok(())
}
