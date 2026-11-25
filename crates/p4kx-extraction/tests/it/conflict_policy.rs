// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Extracting into a folder that already holds files, under each Conflict policy.

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, ConflictPolicy, ExtractOptions, ExtractionReport, extract};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

type TestResult = Result<(), Box<dyn Error>>;

struct Prefilled {
    _dir: tempfile::TempDir,
    archive: Archive,
    target: std::path::PathBuf,
}

fn prefilled() -> Result<Prefilled, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\same.txt", b"same".to_vec()))
        .entry(TestEntry::deflated("Data\\edited.txt", b"new!".to_vec()))
        .entry(TestEntry::zstd("Data\\resized.txt", b"longer".to_vec()).encrypted())
        .entry(TestEntry::stored("Data\\fresh.txt", b"fresh".to_vec()))
        .write_to(&path)?;
    let target = dir.path().join("out");
    fs::create_dir_all(target.join("Data"))?;
    fs::write(target.join("Data").join("same.txt"), "same")?;
    fs::write(target.join("Data").join("edited.txt"), "old!")?;
    fs::write(target.join("Data").join("resized.txt"), "short")?;
    Ok(Prefilled {
        archive: Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?,
        target,
        _dir: dir,
    })
}

fn run(fixture: &Prefilled, policy: ConflictPolicy) -> ExtractionReport {
    let entries: Vec<_> = fixture.archive.entries().iter().collect();
    let options = ExtractOptions {
        policy,
        ..ExtractOptions::default()
    };
    extract(&fixture.archive, &entries, &fixture.target, &options)
}

fn contents(target: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    ["same.txt", "edited.txt", "resized.txt", "fresh.txt"]
        .iter()
        .map(|name| Ok(fs::read_to_string(target.join("Data").join(name))?))
        .collect()
}

#[test]
fn skip_is_the_default_policy() {
    assert_eq!(
        ExtractOptions::default().policy,
        ConflictPolicy::Skip,
        "existing files are kept unless asked otherwise"
    );
}

#[test]
fn skip_keeps_every_existing_file() -> TestResult {
    let fixture = prefilled()?;

    let report = run(&fixture, ConflictPolicy::Skip);

    assert_eq!(report.extracted(), 1, "only the new Entry is written");
    assert_eq!(
        report.skipped(),
        ["Data\\same.txt", "Data\\edited.txt", "Data\\resized.txt"],
        "every existing file is skipped"
    );
    assert_eq!(
        contents(&fixture.target)?,
        ["same", "old!", "short", "fresh"],
        "existing files are unchanged"
    );
    assert!(report.is_complete(), "skipping is not a failure");
    Ok(())
}

#[test]
fn overwrite_replaces_every_existing_file() -> TestResult {
    let fixture = prefilled()?;

    let report = run(&fixture, ConflictPolicy::Overwrite);

    assert_eq!(report.extracted(), 4, "every Entry is written");
    assert!(report.skipped().is_empty(), "nothing is skipped");
    assert_eq!(
        contents(&fixture.target)?,
        ["same", "new!", "longer", "fresh"],
        "every file holds the Entry data"
    );
    assert!(report.is_complete(), "overwriting is not a failure");
    Ok(())
}

#[test]
fn if_changed_replaces_only_files_whose_size_or_crc_differs() -> TestResult {
    let fixture = prefilled()?;

    let report = run(&fixture, ConflictPolicy::IfChanged);

    assert_eq!(
        report.extracted(),
        3,
        "the changed and the new Entries are written"
    );
    assert_eq!(
        report.skipped(),
        ["Data\\same.txt"],
        "the unchanged file is skipped"
    );
    assert_eq!(
        contents(&fixture.target)?,
        ["same", "new!", "longer", "fresh"],
        "every file holds the Entry data"
    );
    assert!(report.is_complete(), "skipping is not a failure");
    Ok(())
}

#[test]
fn every_policy_leaves_no_temporary_file_behind() -> TestResult {
    for (policy, companions) in [
        (ConflictPolicy::Skip, &["fresh.txt.p4kx-origin.jsonl"][..]),
        (
            ConflictPolicy::Overwrite,
            &[
                "edited.txt.p4kx-origin.jsonl",
                "fresh.txt.p4kx-origin.jsonl",
                "resized.txt.p4kx-origin.jsonl",
                "same.txt.p4kx-origin.jsonl",
            ][..],
        ),
        (
            ConflictPolicy::IfChanged,
            &[
                "edited.txt.p4kx-origin.jsonl",
                "fresh.txt.p4kx-origin.jsonl",
                "resized.txt.p4kx-origin.jsonl",
            ][..],
        ),
    ] {
        let fixture = prefilled()?;

        let _report = run(&fixture, policy);

        let mut names: Vec<_> = fs::read_dir(fixture.target.join("Data"))?
            .map(|item| Ok(item?.file_name().to_string_lossy().into_owned()))
            .collect::<Result<_, std::io::Error>>()?;
        names.sort();
        let mut expected = vec!["edited.txt", "fresh.txt", "resized.txt", "same.txt"];
        expected.extend_from_slice(companions);
        expected.sort_unstable();
        assert_eq!(
            names, expected,
            "{policy:?}: only the Entry files and expected companions are in the folder"
        );
    }
    Ok(())
}
