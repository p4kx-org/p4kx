// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The unresolved layer files in the JSON reports of `entity glb`, `armour glb` and
//! `interior glb`.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, INTERIOR_ROOT, TestEntry, TestMtl, TestMtlMaterial, armour_entries,
    interior_entries, layered_entries,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const ABSENT: &str = "materials/layers/absent.mtl";

fn write_archive(dir: &Path, entries: Vec<TestEntry>) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(path)
}

/// Replaces the library `name` of `entries` by one whose layers name a file the Archive lacks.
fn with_absent_layer(entries: Vec<TestEntry>, name: &str) -> Vec<TestEntry> {
    let layered = TestMtlMaterial::new("hull")
        .shader("HardSurface")
        .layer_with(&[("Name", "Primary"), ("Path", ABSENT)]);
    let mut kept: Vec<TestEntry> = entries
        .into_iter()
        .filter(|entry| !entry.path().ends_with(name))
        .collect();
    kept.push(TestEntry::stored(
        format!("Data\\objects\\test\\{name}"),
        TestMtl::multi(&[layered.clone(), layered.clone(), layered]).cryxml(),
    ));
    kept
}

/// Runs `p4kx --json` with `args` and returns its one JSON document.
fn json(args: &[&Path], words: &[&str]) -> Result<Value, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command.arg("--json").args(words).args(args);
    let output = command.output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    Ok(serde_json::from_slice(&output.stdout)?)
}

#[test]
fn entity_glb_reports_the_layer_files_its_archive_lacks() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), layered_entries())?;
    let out = dir.path().join("ship.glb");

    let report = json(
        &[
            &archive,
            Path::new("TEST_Layered"),
            Path::new("--out"),
            &out,
        ],
        &["entity", "glb"],
    )?;

    assert_eq!(report["unresolved_layers"], 1);
    assert_eq!(
        report["first_unresolved_layer"],
        "Objects/Test/layers/lost.mtl"
    );
    Ok(())
}

#[test]
fn entity_glb_without_a_missing_layer_file_reports_none() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), p4kx_testkit::assembly_entries())?;
    let out = dir.path().join("ship.glb");

    let report = json(
        &[&archive, Path::new("TEST_Ship"), Path::new("--out"), &out],
        &["entity", "glb"],
    )?;

    assert_eq!(report["unresolved_layers"], 0);
    assert!(report.get("first_unresolved_layer").is_none());
    Ok(())
}

#[test]
fn armour_glb_reports_the_layer_files_its_archive_lacks() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), with_absent_layer(armour_entries(), "cube.mtl"))?;
    let out = dir.path().join("armour.glb");

    let report = json(
        &[
            &archive,
            Path::new("--piece"),
            Path::new("TEST_Plain"),
            Path::new("--out"),
            &out,
        ],
        &["armour", "glb"],
    )?;

    assert_eq!(report["unresolved_layers"], 1);
    assert_eq!(report["first_unresolved_layer"], ABSENT);
    Ok(())
}

#[test]
fn interior_glb_reports_the_layer_files_its_archive_lacks() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(
        dir.path(),
        with_absent_layer(interior_entries(), "ship.mtl"),
    )?;
    let out = dir.path().join("interior.glb");

    let report = json(
        &[&archive, Path::new(INTERIOR_ROOT), Path::new("--out"), &out],
        &["interior", "glb"],
    )?;

    assert_eq!(report["unresolved_layers"], 1);
    assert_eq!(report["first_unresolved_layer"], ABSENT);
    Ok(())
}

#[test]
fn two_parts_sharing_an_unresolved_layer_file_count_it_once() -> TestResult {
    let dir = tempfile::tempdir()?;
    let entries = with_absent_layer(p4kx_testkit::assembly_entries(), "ship.mtl");
    let archive = write_archive(dir.path(), entries)?;
    let out = dir.path().join("ship.glb");

    let report = json(
        &[&archive, Path::new("TEST_Ship"), Path::new("--out"), &out],
        &["entity", "glb"],
    )?;

    assert_eq!(report["unresolved_layers"], 1, "the ship and its mount");
    assert_eq!(report["first_unresolved_layer"], ABSENT);
    Ok(())
}

#[test]
fn the_interior_of_an_entity_adds_its_unresolved_layer_files() -> TestResult {
    let dir = tempfile::tempdir()?;
    let entries = with_absent_layer(p4kx_testkit::entity_interior_entries(), "ship.mtl");
    let archive = write_archive(dir.path(), entries)?;
    let out = dir.path().join("hull.glb");
    let class = Path::new(p4kx_testkit::ENTITY_INTERIOR_CLASS);

    let without = json(
        &[&archive, class, Path::new("--out"), &out],
        &["entity", "glb"],
    )?;
    let with = json(
        &[
            &archive,
            class,
            Path::new("--out"),
            &out,
            Path::new("--interior"),
        ],
        &["entity", "glb"],
    )?;

    assert_eq!(without["unresolved_layers"], 0, "the hull alone");
    assert_eq!(with["unresolved_layers"], 1, "the containers add theirs");
    assert_eq!(with["first_unresolved_layer"], ABSENT);
    Ok(())
}
