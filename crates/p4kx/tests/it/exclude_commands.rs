// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `--exclude` of `p4kx verify`, `p4kx grep`, `p4kx convert` and `p4kx socpak`, and the bounds on
//! Filter text of the commands that take a Filter, over synthetic Archives.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use p4kx_testkit::{ArchiveBuilder, SplitDds, TestContainer, TestEntry, TestTextureFormat, p4kx};
use predicates::prelude::{PredicateBooleanExt, predicate};

type TestResult = Result<(), Box<dyn Error>>;

const DATA: &[u8] = b"payload";
const PANEL: &str = "Data\\Textures\\panel_diff.dds";

fn archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    SplitDds::new(TestTextureFormat::LegacyDxt1, 16, 8)
        .split(2)
        .entries(PANEL)
        .into_iter()
        .chain([
            TestEntry::stored("Data\\good.bin", DATA),
            TestEntry::deflated("Data\\bad.bin", DATA).with_crc32c(0x1234_5678),
            TestEntry::stored(
                "Data\\Scripts\\ship.xml",
                b"<Ship name=\"gladius\"/>".to_vec(),
            ),
            TestEntry::stored(
                "Data\\Scripts\\hull.xml",
                b"<Hull name=\"gladius\"/>".to_vec(),
            ),
        ])
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(path)
}

fn names(root: &Path) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let mut found = BTreeSet::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else {
                found.insert(
                    path.strip_prefix(root)?
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    Ok(found)
}

#[test]
fn verify_leaves_out_what_an_exclude_matches() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;

    p4kx()?
        .arg("verify")
        .arg(&archive)
        .args([
            "--filter",
            "Data\\",
            "--exclude",
            "bad",
            "--exclude",
            ".dds",
        ])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("verified 3, mismatched 0"));
    p4kx()?
        .arg("verify")
        .arg(&archive)
        .args(["--exclude", "bad", "--exclude", ".dds"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("verified 3, mismatched 0"));
    Ok(())
}

#[test]
fn verify_with_excludes_that_leave_nothing_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;

    p4kx()?
        .arg("verify")
        .arg(&archive)
        .args(["--filter", "good", "--exclude", "data"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "the exclusion Filters leave out all 1 Entries that good matches",
        ));
    p4kx()?
        .arg("verify")
        .arg(&archive)
        .args(["--filter", "no-such-entry", "--exclude", "data"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no Entry matches no-such-entry"));
    Ok(())
}

#[test]
fn grep_leaves_out_what_an_exclude_matches() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;

    p4kx()?
        .arg("grep")
        .arg(&archive)
        .arg("gladius")
        .args(["--in", "entries", "--exclude", "ship"])
        .assert()
        .code(0)
        .stdout(
            predicate::str::contains("hull.xml").and(predicate::str::contains("ship.xml").not()),
        );
    p4kx()?
        .arg("grep")
        .arg(&archive)
        .arg("gladius")
        .args([
            "--in",
            "entries",
            "--filter",
            "Scripts",
            "--exclude",
            "hull",
        ])
        .assert()
        .code(0)
        .stdout(
            predicate::str::contains("ship.xml").and(predicate::str::contains("hull.xml").not()),
        );
    Ok(())
}

#[test]
fn socpak_extract_takes_an_exclude_without_a_filter() -> TestResult {
    let dir = tempfile::tempdir()?;
    let container = TestContainer::new()
        .entry(TestEntry::stored("room\\good.xml", "<Good/>"))
        .entry(TestEntry::stored("room\\bad.xml", "<Bad/>"));
    let archive = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\room.socpak", container.build()))
        .write_to(&archive)?;
    let out = dir.path().join("out");

    p4kx()?
        .args(["socpak", "extract"])
        .arg(&archive)
        .arg("Data\\room.socpak")
        .args(["--exclude", "bad", "--out"])
        .arg(&out)
        .assert()
        .code(0);
    assert_eq!(names(&out)?, BTreeSet::from(["room/good.xml".to_owned()]));
    p4kx()?
        .args(["socpak", "extract"])
        .arg(&archive)
        .arg("Data\\room.socpak")
        .args(["--exclude", "room", "--out"])
        .arg(dir.path().join("none"))
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "leave out all 2 Entries that * matches",
        ));
    Ok(())
}

#[test]
fn socpak_verify_and_extract_take_an_exclude() -> TestResult {
    let dir = tempfile::tempdir()?;
    let container = TestContainer::new()
        .entry(TestEntry::stored("room\\good.xml", "<Good/>"))
        .entry(TestEntry::deflated("room\\bad.xml", "<Bad/>").with_crc32(0xDEAD_BEEF));
    let archive = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\room.socpak", container.build()))
        .write_to(&archive)?;

    p4kx()?
        .args(["socpak", "verify"])
        .arg(&archive)
        .arg("Data\\room.socpak")
        .args(["--exclude", "bad"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("verified 1, mismatched 0"));
    let out = dir.path().join("out");
    p4kx()?
        .args(["socpak", "extract"])
        .arg(&archive)
        .arg("Data\\room.socpak")
        .args(["--filter", "room", "--exclude", "bad", "--out"])
        .arg(&out)
        .assert()
        .code(0);
    assert_eq!(names(&out)?, BTreeSet::from(["room/good.xml".to_owned()]));
    Ok(())
}

fn extracted(dir: &Path, archive: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let raw = dir.join("raw");
    p4kx()?
        .arg("extract")
        .arg(archive)
        .args(["--filter", "Data", "--out"])
        .arg(&raw)
        .assert()
        .code(0);
    Ok(raw)
}

fn convert(raw: &Path, out: &Path) -> Result<Command, Box<dyn Error>> {
    let mut command = p4kx()?;
    command
        .arg("convert")
        .arg(raw)
        .arg("--copy-other")
        .arg("--out")
        .arg(out);
    Ok(command)
}

#[test]
fn convert_leaves_out_a_texture_with_its_parts_when_its_header_is_excluded() -> TestResult {
    let dir = tempfile::tempdir()?;
    let raw = extracted(dir.path(), &archive(dir.path())?)?;
    let out = dir.path().join("out");

    convert(&raw, &out)?
        .args(["--exclude", "panel_diff.dds"])
        .assert()
        .code(0);

    let written = names(&out)?;
    assert!(
        written.iter().all(|name| !name.contains("panel_diff")),
        "no file of the texture, no stray part: {written:?}"
    );
    assert!(
        written.contains("Data/good.bin"),
        "the rest is copied: {written:?}"
    );
    Ok(())
}

#[test]
fn convert_notes_an_excluded_part_of_a_kept_texture() -> TestResult {
    let dir = tempfile::tempdir()?;
    let raw = extracted(dir.path(), &archive(dir.path())?)?;
    let out = dir.path().join("out");

    convert(&raw, &out)?
        .args(["--exclude", "*.dds.?"])
        .assert()
        .code(0)
        .stderr(predicate::str::contains(
            "note: --exclude left out 2 part Entries of textures that stay",
        ));

    let written = names(&out)?;
    assert!(
        written.iter().any(|name| name.ends_with("panel_diff.png")),
        "the texture is built from all its parts: {written:?}"
    );
    let siblings = written
        .iter()
        .filter_map(|name| name.strip_prefix("Data/Textures/panel_diff.dds."));
    assert!(
        siblings.eq(["p4kx-origin.jsonl"]),
        "the merged receipt is the only DDS sibling: {written:?}"
    );
    Ok(())
}

#[test]
fn convert_refuses_a_filter_pair_that_selects_nothing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let raw = extracted(dir.path(), &archive(dir.path())?)?;
    let out = dir.path().join("out");

    convert(&raw, &out)?
        .args(["--filter", "panel", "--exclude", "TEXTURES"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "the exclusion Filters leave out all 3 Entries that panel matches",
        ));
    convert(&raw, &out)?
        .args(["--filter", "no-such-file", "--exclude", "panel"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no Entry matches no-such-file"));
    convert(&raw, &out)?
        .args(["--exclude", "data"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("that * matches"));
    assert!(!out.exists(), "nothing was converted");
    Ok(())
}
