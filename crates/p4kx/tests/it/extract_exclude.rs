// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx extract` and `p4kx search` with `--exclude` over a synthetic Archive.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use p4kx_testkit::{ArchiveBuilder, SplitDds, TestEntry, TestTextureFormat};
use predicates::prelude::predicate;

type TestResult = Result<(), Box<dyn Error>>;

const PANEL: &str = "Data\\Textures\\panel_diff.dds";

struct Fixture {
    _dir: tempfile::TempDir,
    archive: PathBuf,
    out: PathBuf,
}

fn fixture() -> Result<Fixture, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    SplitDds::new(TestTextureFormat::LegacyDxt1, 16, 8)
        .split(2)
        .entries(PANEL)
        .into_iter()
        .chain([
            TestEntry::stored("Data\\Objects\\ship.cga", b"hull".to_vec()),
            TestEntry::stored("Data\\Objects\\ship_lod1.cgam", b"one".to_vec()),
            TestEntry::stored("Data\\Objects\\ship_lod2.cgam", b"two".to_vec()),
            TestEntry::stored("Data\\Objects\\Sub\\part.cga", b"part".to_vec()),
            TestEntry::stored("Data\\readme.txt", b"hello".to_vec()),
        ])
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&archive)?;
    let out = dir.path().join("out");
    Ok(Fixture {
        _dir: dir,
        archive,
        out,
    })
}

fn extract(fixture: &Fixture, filter: &str) -> Result<Command, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command
        .arg("extract")
        .arg(&fixture.archive)
        .args(["--filter", filter])
        .arg("--out")
        .arg(&fixture.out);
    Ok(command)
}

fn files(root: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut found = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(&folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else {
                let relative = path
                    .strip_prefix(root)?
                    .to_string_lossy()
                    .replace('\\', "/");
                found.push(relative);
            }
        }
    }
    found.sort();
    Ok(found)
}

#[test]
fn extract_leaves_out_what_an_exclude_matches() -> TestResult {
    let fixture = fixture()?;

    extract(&fixture, "Data\\Objects\\")?
        .args(["--exclude", "*_lod?.cgam"])
        .assert()
        .code(0);

    assert_eq!(
        files(&fixture.out)?,
        [
            "Data/Objects/Sub/part.cga",
            "Data/Objects/Sub/part.cga.p4kx-origin.jsonl",
            "Data/Objects/ship.cga",
            "Data/Objects/ship.cga.p4kx-origin.jsonl",
        ],
        "the level-of-detail companions are not written"
    );
    Ok(())
}

#[test]
fn extract_takes_several_excludes_ignoring_case_and_separator_kind() -> TestResult {
    let fixture = fixture()?;

    extract(&fixture, "objects")?
        .args(["--exclude", "OBJECTS/sub", "--exclude", "_LOD1"])
        .assert()
        .code(0);

    assert_eq!(
        files(&fixture.out)?,
        [
            "Data/Objects/ship.cga",
            "Data/Objects/ship.cga.p4kx-origin.jsonl",
            "Data/Objects/ship_lod2.cgam",
            "Data/Objects/ship_lod2.cgam.p4kx-origin.jsonl",
        ],
        "each exclude removes its own Entries"
    );
    Ok(())
}

#[test]
fn a_filter_that_matches_nothing_is_invalid_input_even_with_an_exclude() -> TestResult {
    let fixture = fixture()?;

    extract(&fixture, "no-such-entry")?
        .args(["--exclude", "ship"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no Entry matches no-such-entry"));
    Ok(())
}

#[test]
fn excludes_that_leave_nothing_are_invalid_input_and_say_so() -> TestResult {
    let fixture = fixture()?;

    extract(&fixture, "Data\\Objects\\")?
        .args(["--exclude", "ship", "--exclude", "part"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "the exclusion Filters leave out all 4 Entries that Data\\Objects\\ matches",
        ));

    assert!(!fixture.out.exists(), "nothing was extracted");
    Ok(())
}

#[test]
fn invalid_filter_text_is_invalid_input() -> TestResult {
    let fixture = fixture()?;
    let long = "a".repeat(2000);
    let wild = "*a".repeat(40);

    extract(&fixture, "Data")?
        .args(["--exclude", &long])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("2000 bytes long"));
    extract(&fixture, "Data")?
        .args(["--exclude", &wild])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("40 wildcards"));
    extract(&fixture, &long)?
        .assert()
        .code(2)
        .stderr(predicate::str::contains("2000 bytes long"));
    extract(&fixture, "Data")?
        .args(["--exclude", ""])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("exclusion Filter is empty"));
    Ok(())
}

#[test]
fn an_exclude_needs_a_filter() -> TestResult {
    let fixture = fixture()?;

    p4kx_testkit::p4kx()?
        .arg("extract")
        .arg(&fixture.archive)
        .arg("Data\\readme.txt")
        .args(["--exclude", "readme"])
        .assert()
        .code(2);
    Ok(())
}

#[test]
fn convert_leaves_out_a_texture_with_its_parts_when_its_header_is_excluded() -> TestResult {
    let fixture = fixture()?;

    extract(&fixture, "Data\\")?
        .args(["--convert", "--exclude", "panel_diff.dds"])
        .assert()
        .code(0);

    let written = files(&fixture.out)?;
    assert!(
        written.iter().all(|path| !path.contains("panel_diff")),
        "no file of the texture, no stray part: {written:?}"
    );
    assert!(
        written.contains(&"Data/readme.txt".to_owned()),
        "the other Entries are converted and written: {written:?}"
    );
    Ok(())
}

#[test]
fn convert_reports_an_excluded_part_of_a_kept_texture() -> TestResult {
    let fixture = fixture()?;

    extract(&fixture, "Data\\")?
        .args(["--convert", "--exclude", "*.dds.?"])
        .assert()
        .code(0)
        .stderr(predicate::str::contains(
            "note: --exclude left out 2 part Entries of textures that stay",
        ));

    let written = files(&fixture.out)?;
    assert!(
        written.contains(&"Data/Textures/panel_diff.dds".to_owned()),
        "the texture is still merged from its parts: {written:?}"
    );
    assert!(
        written.iter().any(|path| {
            Path::new(path)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
        }),
        "and exported: {written:?}"
    );
    Ok(())
}

#[test]
fn search_leaves_out_what_an_exclude_matches() -> TestResult {
    let fixture = fixture()?;

    let output = p4kx_testkit::p4kx()?
        .arg("--json")
        .arg("search")
        .arg(&fixture.archive)
        .arg("Objects")
        .args(["--exclude", "_lod", "--exclude", "Sub"])
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let paths: Vec<&str> = json["entries"]
        .as_array()
        .ok_or("entries")?
        .iter()
        .filter_map(|entry| entry["path"].as_str())
        .collect();
    assert_eq!(paths, ["Data\\Objects\\ship.cga"], "only the ship remains");
    assert_eq!(
        json["exclude"],
        serde_json::json!(["_lod", "Sub"]),
        "the exclusion Filters are echoed"
    );
    Ok(())
}

#[test]
fn search_with_an_exclude_that_leaves_nothing_is_invalid_input() -> TestResult {
    let fixture = fixture()?;

    p4kx_testkit::p4kx()?
        .arg("search")
        .arg(&fixture.archive)
        .arg("readme")
        .args(["--exclude", "data"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "the exclusion Filters leave out all 1 Entries that readme matches",
        ));
    Ok(())
}

#[test]
fn search_without_an_exclude_keeps_an_empty_result_a_result() -> TestResult {
    let fixture = fixture()?;

    p4kx_testkit::p4kx()?
        .arg("search")
        .arg(&fixture.archive)
        .arg("no-such-entry")
        .assert()
        .code(0);
    Ok(())
}
