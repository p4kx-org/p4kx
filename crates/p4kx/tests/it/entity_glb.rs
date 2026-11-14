// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx entity glb` end to end over the synthetic ship of the testkit.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::gltf::Node;
use p4kx_testkit::{ArchiveBuilder, CheckedGlb, TestEntry, assembly_entries, check_glb};

type TestResult = Result<(), Box<dyn Error>>;

struct Run {
    stdout: String,
    stderr: String,
    code: Option<i32>,
}

fn write_archive(dir: &Path, entries: Vec<TestEntry>) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(path)
}

fn glb(archive: &Path, out: &Path, extra: &[&str], json: bool) -> Result<Run, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    if json {
        command.arg("--json");
    }
    let output = command
        .args(["entity", "glb"])
        .arg(archive)
        .arg("TEST_Ship")
        .arg("--out")
        .arg(out)
        .args(extra)
        .output()?;
    Ok(Run {
        stdout: String::from_utf8(output.stdout)?,
        stderr: String::from_utf8(output.stderr)?,
        code: output.status.code(),
    })
}

fn node<'a>(glb: &'a CheckedGlb, name: &str) -> Result<Node<'a>, Box<dyn Error>> {
    glb.document
        .nodes()
        .find(|node| node.name() == Some(name))
        .ok_or_else(|| format!("no node {name}").into())
}

#[test]
fn the_ship_is_written_with_its_loadout_placed_and_what_is_missing_listed() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), assembly_entries())?;
    let out = dir.path().join("ship.glb");

    let run = glb(&archive, &out, &[], false)?;

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let checked = check_glb(&std::fs::read(&out)?)?;
    let mount = node(&checked, "hardpoint_weapon_nose/TEST_Mount")?;
    let (translation, _, _) = mount.transform().decomposed();
    assert!(
        translation
            .iter()
            .zip([0.0, 1.0, -4.0])
            .all(|(a, e)| (a - e).abs() < 1e-5),
        "{translation:?}"
    );
    assert!(
        run.stdout.contains("6 parts (4 with models)"),
        "{}",
        run.stdout
    );
    for expected in [
        "skipped hardpoint_cargo ",
        "skipped hardpoint_door (TEST_Door): the geometry Objects/Test/door.cgf does not exist",
        "note hardpoint_seat (TEST_Seat): the Entity names no geometry",
    ] {
        assert!(run.stdout.contains(expected), "{expected}: {}", run.stdout);
    }
    Ok(())
}

#[test]
fn the_json_report_has_a_stable_shape() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), assembly_entries())?;
    let out = dir.path().join("ship.glb");

    let run = glb(&archive, &out, &["--lod", "1"], true)?;

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    assert_eq!(run.stdout.lines().count(), 1, "one line of JSON");
    let mut shaped: serde_json::Value = serde_json::from_str(&run.stdout)?;
    for key in ["archive", "out"] {
        shaped[key] = serde_json::Value::String(format!("<{key}>"));
    }
    insta::assert_snapshot!(serde_json::to_string_pretty(&shaped)?);
    let checked = check_glb(&std::fs::read(&out)?)?;
    assert_eq!(
        checked.document.nodes().len(),
        17,
        "6 part nodes and the 11 nodes of 4 models"
    );
    Ok(())
}

#[test]
fn physical_fallback_preserves_the_authored_library_failure() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut entries = assembly_entries();
    entries.retain(|entry| !entry.path().ends_with("ship.mtl"));
    entries.push(TestEntry::stored(
        "Data/Materials/material_default.mtl",
        br#"<Material Name="archive-fallback" Shader="Illum" Diffuse="0.1,0.2,0.3"/>"#,
    ));
    let archive = write_archive(dir.path(), entries)?;
    let out = dir.path().join("physical.glb");
    let run = glb(&archive, &out, &[], true)?;
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let report: serde_json::Value = serde_json::from_str(&run.stdout)?;
    assert_eq!(
        report["default_material"]["state"], "resolved",
        "actual archive lookup"
    );
    assert_eq!(
        report["default_material"]["source"]["contains_encrypted_source"], false,
        "the original fallback Entry is unencrypted"
    );
    assert!(
        report["issues"]
            .as_array()
            .ok_or("assembly issues")?
            .iter()
            .any(|issue| issue["kind"] == "material_library"),
        "authored library remains independently missing"
    );
    let glb = check_glb(&std::fs::read(&out)?)?;
    assert!(
        glb.document
            .materials()
            .any(|material| material.name() == Some("archive-fallback")),
        "the combined scene uses the physical material"
    );
    Ok(())
}

#[test]
fn an_output_that_is_the_archive_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), assembly_entries())?;
    let before = std::fs::read(&archive)?;

    let run = glb(&archive, &archive, &[], false)?;

    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(run.stderr.contains("is the Archive"), "{}", run.stderr);
    assert_eq!(std::fs::read(&archive)?, before, "the Archive is untouched");
    Ok(())
}

#[test]
fn a_level_beyond_the_probed_ones_is_a_usage_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), assembly_entries())?;
    let out = dir.path().join("ship.glb");

    let rejected = glb(&archive, &out, &["--lod", "16"], false)?;
    let accepted = glb(&archive, &out, &["--lod", "15"], false)?;

    assert_eq!(rejected.code, Some(2), "{}", rejected.stderr);
    assert!(rejected.stderr.contains("--lod"), "{}", rejected.stderr);
    assert_eq!(accepted.code, Some(0), "{}", accepted.stderr);
    Ok(())
}
#[test]
fn an_unknown_entity_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), assembly_entries())?;
    let out = dir.path().join("ship.glb");

    let output = p4kx_testkit::p4kx()?
        .args(["entity", "glb"])
        .arg(&archive)
        .arg("No_Such_Ship")
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(2));
    assert!(!out.exists(), "nothing written");
    Ok(())
}

#[test]
fn an_encrypted_model_without_the_key_exits_with_7_and_writes_nothing() -> TestResult {
    let encrypted: Vec<TestEntry> = assembly_entries()
        .into_iter()
        .map(|entry| {
            if format!("{entry:?}").contains("ship.cgfm") {
                TestEntry::stored("Data\\Objects\\Test\\ship.cgfm", Vec::<u8>::new()).encrypted()
            } else {
                entry
            }
        })
        .collect();
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), encrypted)?;
    let out = dir.path().join("ship.glb");

    let output = p4kx_testkit::p4kx()?
        .env_remove("P4KX_ARCHIVE_KEY")
        .env(
            "P4KX_KEY_FILE",
            dir.path().join("profile").join("archive.key"),
        )
        .args(["entity", "glb"])
        .arg(&archive)
        .arg("TEST_Ship")
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(7), "{output:?}");
    assert!(!out.exists(), "nothing written");
    Ok(())
}
