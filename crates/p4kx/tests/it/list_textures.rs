// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx list` and `p4kx extract --filter` with split textures.

use std::error::Error;
use std::path::Path;

use p4kx_testkit::{ArchiveBuilder, TestEntry};

type TestResult = Result<(), Box<dyn Error>>;

fn archive(dir: &Path) -> Result<std::path::PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\a.txt", vec![1; 5]))
        .entry(TestEntry::stored("Data\\tex\\lonely.dds", vec![8; 9]))
        .entry(TestEntry::stored("Data\\tex\\orphan.dds.1", vec![2; 7]))
        .entry(TestEntry::stored("Data\\tex\\wall.dds", vec![3; 100]))
        .entry(TestEntry::stored("Data\\tex\\wall.dds.1", vec![4; 20]))
        .entry(TestEntry::stored("Data\\tex\\wall.dds.2", vec![5; 3]))
        .entry(TestEntry::stored("Data\\tex\\wall.dds.a", vec![6; 4]))
        .write_to(&path)?;
    Ok(path)
}

fn list(archive: &Path, extra: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .arg("list")
        .arg(archive)
        .args(extra)
        .output()?;
    assert!(output.status.success(), "list succeeds: {output:?}");
    Ok(String::from_utf8(output.stdout)?)
}

#[test]
fn list_folds_the_parts_of_a_texture_into_the_row_of_its_header() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;

    let stdout = list(&archive, &[])?;

    let rows: Vec<Vec<&str>> = stdout
        .lines()
        .skip(1)
        .map(|line| line.split_whitespace().collect())
        .collect();
    assert_eq!(
        rows,
        vec![
            vec!["store", "no", "5", "5", "Data\\a.txt"],
            vec!["store", "no", "9", "9", "Data\\tex\\lonely.dds"],
            vec!["store", "no", "7", "7", "Data\\tex\\orphan.dds.1"],
            vec![
                "store",
                "no",
                "127",
                "127",
                "Data\\tex\\wall.dds",
                "(+3",
                "parts)"
            ],
        ],
        "one row per texture with the total size; a header without parts and an orphaned part stay single rows"
    );
    Ok(())
}

#[test]
fn list_parts_lists_every_part_as_an_entry() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;

    let stdout = list(&archive, &["--parts"])?;

    assert_eq!(
        stdout.lines().count(),
        1 + 7,
        "the header line and seven Entries"
    );
    assert!(stdout.contains("Data\\tex\\wall.dds.2"), "{stdout}");
    assert!(!stdout.contains("parts)"), "no folded row: {stdout}");
    Ok(())
}

#[test]
fn list_json_gives_the_parts_of_a_texture_and_its_total_sizes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;

    let stdout = list(&archive, &["--json"])?;

    let value: serde_json::Value = serde_json::from_str(&stdout)?;
    let entries = value["entries"].as_array().ok_or("entries")?;
    assert_eq!(entries.len(), 4, "four rows");
    let wall = &entries[3];
    assert_eq!(wall["path"], "Data\\tex\\wall.dds");
    assert_eq!(wall["size"], 127, "total size");
    assert_eq!(wall["compressed_size"], 127, "total stored size");
    assert_eq!(
        wall["metadata"]["owner_entry_index"], 3,
        "physical base owner"
    );
    assert_eq!(
        wall["metadata"]["original_directory_ordinal"], "3",
        "base source ordinal"
    );
    assert_eq!(
        wall["parts"],
        serde_json::json!([
            "Data\\tex\\wall.dds.1",
            "Data\\tex\\wall.dds.2",
            "Data\\tex\\wall.dds.a"
        ]),
        "the parts in order"
    );
    assert!(entries[0].get("parts").is_none(), "a plain Entry has none");
    assert!(entries[1].get("parts").is_none(), "a header without parts");

    let all: serde_json::Value = serde_json::from_str(&list(&archive, &["--json", "--parts"])?)?;
    assert_eq!(all["entries"].as_array().ok_or("entries")?.len(), 7);
    assert_eq!(
        all["entries"][4]["metadata"]["owner_entry_index"], 4,
        "a part owns its metadata"
    );
    assert_eq!(all["entries"][4]["size"], 20, "part sizes stay physical");
    Ok(())
}

#[test]
fn list_json_snapshot() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;

    let stdout = list(&archive, &["--json"])?;

    let mut value: serde_json::Value = serde_json::from_str(&stdout)?;
    value["archive"] = "[DIR]/Data.p4k".into();
    insta::assert_snapshot!(serde_json::to_string_pretty(&value)?);
    Ok(())
}

#[test]
fn a_single_part_reads_part_not_parts() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\one.dds", vec![1; 4]))
        .entry(TestEntry::stored("Data\\one.dds.1", vec![2; 4]))
        .write_to(&path)?;

    let stdout = list(&path, &[])?;

    assert!(stdout.contains("one.dds (+1 part)"), "{stdout}");
    Ok(())
}

#[test]
fn extract_with_a_filter_includes_all_parts_of_a_matching_texture() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let out = dir.path().join("out");

    let output = p4kx_testkit::p4kx()?
        .arg("extract")
        .arg(&archive)
        .args(["--filter", "*wall.dds"])
        .arg("--out")
        .arg(&out)
        .output()?;

    assert!(output.status.success(), "extract succeeds: {output:?}");
    for name in ["wall.dds", "wall.dds.1", "wall.dds.2", "wall.dds.a"] {
        assert!(out.join("Data").join("tex").join(name).is_file(), "{name}");
    }
    assert!(!out.join("Data").join("a.txt").exists(), "not selected");
    Ok(())
}
