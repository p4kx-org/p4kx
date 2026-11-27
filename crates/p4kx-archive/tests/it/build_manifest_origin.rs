// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Original completed manifest reads retained by Archives and containers.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_archive::{Archive, Build, BuildError, BuildManifestOrigin, BuildManifestOutcome};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry, write_build_manifest};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn write_archive(folder: &Path) -> TestResult<PathBuf> {
    let path = folder.join("Data.p4k");
    let container = TestContainer::new()
        .entry(TestEntry::stored("fact.bin", b"fact"))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("child.socpak", container))
        .write_to(&path)?;
    Ok(path)
}

fn write_manifest(folder: &Path) -> TestResult {
    write_build_manifest(
        folder,
        "synthetic-branch",
        "1.2.3",
        "456",
        "synthetic-build",
    )?;
    Ok(())
}

fn origin(archive: &Archive) -> TestResult<&BuildManifestOrigin> {
    Ok(archive
        .build_manifest_observation()
        .origin()
        .ok_or("completed manifest input")?)
}

#[test]
fn parsed_observation_keeps_the_eager_read_before_a_same_value_rewrite() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive_path = write_archive(directory.path())?;
    write_manifest(directory.path())?;
    let path = directory.path().join("build_manifest.id");
    let original_length = fs::metadata(&path)?.len();
    let archive = Archive::open(&archive_path)?;
    let original = origin(&archive)?.clone();
    let build = archive.build().ok_or("original parsed Build")?.clone();
    let mut bytes = fs::read(&path)?;
    bytes.extend_from_slice(b"\n\n  ");
    fs::write(&path, bytes)?;

    assert!(
        fs::metadata(&path)?.len() > original_length,
        "fixture rewrite changes the original byte count"
    );
    assert_eq!(
        Build::read_from(directory.path())?,
        Some(build.clone()),
        "the rewrite preserves all parsed values"
    );
    assert_eq!(archive.build(), Some(&build), "the eager Build is retained");
    assert_eq!(
        archive.build_manifest_observation().outcome(),
        BuildManifestOutcome::Parsed,
        "the cached eager read parsed successfully"
    );
    assert_eq!(
        origin(&archive)?.name(),
        "build_manifest.id",
        "only the basename is exposed"
    );
    assert_eq!(
        origin(&archive)?.byte_length(),
        original_length,
        "the getter never reopens the changed manifest"
    );
    assert!(
        original.is_same_input(origin(&archive)?),
        "the original completed-read owner is retained"
    );
    Ok(())
}

#[test]
fn equal_build_values_and_paths_do_not_merge_independent_reads() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = write_archive(directory.path())?;
    write_manifest(directory.path())?;
    let first = Archive::open(&path)?;
    let second = Archive::open(&path)?;
    let cloned_observation = first.build_manifest_observation().clone();
    let cloned_origin = origin(&first)?.clone();

    assert_eq!(
        first.build(),
        second.build(),
        "independent reads have equal Build values"
    );
    assert_eq!(
        origin(&first)?.byte_length(),
        origin(&second)?.byte_length(),
        "same bytes do not grant identity"
    );
    assert!(
        cloned_origin.is_same_input(origin(&first)?),
        "a cloned handle retains its owner"
    );
    assert!(
        cloned_observation
            .origin()
            .ok_or("cloned completed read")?
            .is_same_input(origin(&first)?),
        "a cloned observation retains the completed read"
    );
    assert!(
        !origin(&first)?.is_same_input(origin(&second)?),
        "reopening the same path creates a separate original read"
    );
    let patched = first
        .build()
        .ok_or("parsed Build")?
        .clone()
        .with_public_patch(Some("public-patch".to_owned()));
    assert_eq!(
        patched.public_patch(),
        Some("public-patch"),
        "public Build value enrichment is unchanged"
    );
    assert_eq!(
        first.build().ok_or("unmodified Build")?.public_patch(),
        None,
        "patching a value does not change the cached Build"
    );
    assert!(
        cloned_origin.is_same_input(origin(&first)?),
        "a cloned handle retains its owner"
    );
    Ok(())
}

#[test]
fn missing_and_io_failure_have_distinct_outcomes_without_completed_origins() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = write_archive(directory.path())?;
    let missing = Archive::open(&path)?;
    assert_eq!(
        missing.build_manifest_observation().outcome(),
        BuildManifestOutcome::Missing,
        "absence is distinct from a failed read"
    );
    assert!(missing.build().is_none(), "a missing manifest has no Build");
    assert!(
        missing.build_manifest_observation().origin().is_none(),
        "absence cannot create completed-read authority"
    );
    assert!(
        Build::read_from(directory.path())?.is_none(),
        "the public missing-file result is unchanged"
    );

    let manifest_path = directory.path().join("build_manifest.id");
    fs::create_dir(&manifest_path)?;
    let unreadable = Archive::open(&path)?;
    assert_eq!(
        unreadable.build_manifest_observation().outcome(),
        BuildManifestOutcome::Unreadable,
        "an existing directory is an I/O failure"
    );
    assert!(unreadable.build().is_none(), "an I/O failure has no Build");
    assert!(
        unreadable.build_manifest_observation().origin().is_none(),
        "an incomplete read grants no source owner"
    );
    assert!(
        matches!(
            Build::read_from(directory.path()),
            Err(BuildError::Io { path, .. }) if path == manifest_path
        ),
        "the public reader preserves its I/O error and path"
    );
    Ok(())
}

#[test]
fn malformed_completed_reads_include_empty_and_non_utf8_bytes() -> TestResult {
    for bytes in [b"".as_slice(), b"{ not json", b"\xff", br#"{"Data":{}}"#] {
        let directory = tempfile::tempdir()?;
        let path = write_archive(directory.path())?;
        let manifest_path = directory.path().join("build_manifest.id");
        fs::write(&manifest_path, bytes)?;
        let archive = Archive::open(&path)?;

        assert!(
            archive.build().is_none(),
            "malformed completed bytes have no parsed Build"
        );
        assert_eq!(
            archive.build_manifest_observation().outcome(),
            BuildManifestOutcome::Malformed,
            "successful I/O is distinguished from parser failure"
        );
        assert_eq!(
            origin(&archive)?.byte_length(),
            u64::try_from(bytes.len())?,
            "the original byte count includes empty and invalid UTF-8 input"
        );
        assert_eq!(
            origin(&archive)?.name(),
            "build_manifest.id",
            "only the basename is exposed"
        );
        assert!(
            matches!(
                Build::read_from(directory.path()),
                Err(BuildError::Malformed { path, .. }) if path == manifest_path
            ),
            "the standalone reader preserves the original parser error and path"
        );
    }
    Ok(())
}

#[test]
fn containers_inherit_the_original_observation_after_the_manifest_changes() -> TestResult {
    for outcome in [
        BuildManifestOutcome::Missing,
        BuildManifestOutcome::Unreadable,
        BuildManifestOutcome::Malformed,
        BuildManifestOutcome::Parsed,
    ] {
        let directory = tempfile::tempdir()?;
        let path = write_archive(directory.path())?;
        let manifest_path = directory.path().join("build_manifest.id");
        match outcome {
            BuildManifestOutcome::Missing => {}
            BuildManifestOutcome::Unreadable => fs::create_dir(&manifest_path)?,
            BuildManifestOutcome::Malformed => fs::write(&manifest_path, b"invalid")?,
            BuildManifestOutcome::Parsed => write_manifest(directory.path())?,
            _ => return Err("fixture outcome".into()),
        }
        let archive = Archive::open(&path)?;
        if outcome == BuildManifestOutcome::Unreadable {
            fs::remove_dir(&manifest_path)?;
        }
        write_manifest(directory.path())?;
        let entry = archive.entries().first().ok_or("container Entry")?;
        let child = archive.open_container(entry)?;

        assert_eq!(
            archive.build_manifest_observation().outcome(),
            outcome,
            "the root never reclassifies a rewritten manifest"
        );
        assert_eq!(
            child.build_manifest_observation().outcome(),
            outcome,
            "the container inherits the original outcome"
        );
        assert_eq!(
            child.build(),
            archive.build(),
            "the cached Build is shared by value"
        );
        match (
            archive.build_manifest_observation().origin(),
            child.build_manifest_observation().origin(),
        ) {
            (Some(parent), Some(child)) => {
                assert!(
                    parent.is_same_input(child),
                    "the container inherits the exact completed-read owner"
                );
                assert_eq!(
                    parent.byte_length(),
                    child.byte_length(),
                    "the inherited length is unchanged"
                );
            }
            (None, None) => {}
            _ => return Err("container changed original completion evidence".into()),
        }
    }
    Ok(())
}
