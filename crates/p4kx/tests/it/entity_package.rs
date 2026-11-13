// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx entity package` end to end over the synthetic ships of the testkit.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{ArchiveBuilder, TestEntry, assembly_entries, check_glb, livery_entries};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

pub(crate) struct Run {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) code: Option<i32>,
}

pub(crate) fn write_archive(
    dir: &Path,
    entries: Vec<TestEntry>,
) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(path)
}

pub(crate) fn package(
    archive: &Path,
    entity: &str,
    out: &Path,
    extra: &[&str],
    json: bool,
) -> Result<Run, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    if json {
        command.arg("--json");
    }
    let output = command
        .args(["entity", "package"])
        .arg(archive)
        .arg(entity)
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

fn names(folder: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut names: Vec<String> = std::fs::read_dir(folder)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, _>>()?;
    names.sort();
    Ok(names)
}

pub(crate) fn manifest(out: &Path) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(&std::fs::read(
        out.join("scene.json"),
    )?)?)
}

#[test]
fn the_package_has_the_documented_layout_and_validates_against_the_schema() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), livery_entries())?;
    let out = dir.path().join("livery");

    let run = package(
        &archive,
        "TEST_Livery",
        &out,
        &["--texture-mip", "2"],
        false,
    )?;

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    assert_eq!(
        names(&out)?,
        ["meshes", "receipt.json", "scene.json", "textures"]
    );
    assert_eq!(
        names(&out.join("meshes"))?,
        ["m0001.glb", "m0002.glb", "m0003.glb", "m0004.glb"]
    );
    assert_eq!(names(&out.join("textures"))?, ["t0001.png"]);
    let manifest = manifest(&out)?;
    let schema = p4kx_blender::manifest_schema();
    let validator = jsonschema::draft202012::new(&schema)?;
    let violations: Vec<String> = validator
        .iter_errors(&manifest)
        .take(5)
        .map(|error| format!("{} at {}", error, error.instance_path()))
        .collect();
    assert!(violations.is_empty(), "{violations:?}");
    assert_eq!(manifest["format"], "p4kx-blender-package");
    assert_eq!(manifest["version"], p4kx_blender::FORMAT_VERSION);
    assert_eq!(manifest["source"]["entity"]["class_name"], "TEST_Livery");
    for part in manifest["meshes"].as_array().ok_or("parts")? {
        let file = part["file"].as_str().ok_or("file")?;
        check_glb(&std::fs::read(out.join(file))?)?;
    }
    assert!(
        run.stdout
            .contains("7 nodes (7 drawn), 4 meshes, 4 materials, 2 palettes, 1 textures"),
        "{}",
        run.stdout
    );
    Ok(())
}

#[test]
fn shared_meshes_are_written_once_and_the_json_report_says_so() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), livery_entries())?;
    let out = dir.path().join("livery");

    let run = package(&archive, "TEST_Livery", &out, &[], true)?;

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    assert_eq!(run.stdout.lines().count(), 1, "one line of JSON");
    let report: Value = serde_json::from_str(&run.stdout)?;
    assert_eq!(report["nodes"], 7);
    assert_eq!(report["drawn_nodes"], 7);
    assert_eq!(report["meshes"], 4);
    assert_eq!(report["shared_meshes"], 3);
    assert_eq!(report["textures"], 1);
    assert_eq!(report["files"], 7);
    assert_eq!(report["lod"], 0);
    let manifest = manifest(&out)?;
    let instances: Vec<u64> = manifest["meshes"]
        .as_array()
        .ok_or("parts")?
        .iter()
        .filter_map(|part| part["instances"].as_u64())
        .collect();
    assert_eq!(instances, [1, 2, 1, 3]);
    let kinds: Vec<&str> = report["issues"]
        .as_array()
        .ok_or("issues")?
        .iter()
        .filter_map(|issue| issue["kind"].as_str())
        .collect();
    assert_eq!(kinds, ["palette", "palette"]);
    Ok(())
}

#[test]
fn the_level_of_detail_and_the_loadout_changes_are_those_of_entity_glb() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), assembly_entries())?;
    let out = dir.path().join("ship");

    let run = package(
        &archive,
        "TEST_Ship",
        &out,
        &[
            "--lod",
            "1",
            "--exclude",
            "hardpoint_weapon_nose/hardpoint_class_2",
        ],
        true,
    )?;

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let manifest = manifest(&out)?;
    assert_eq!(manifest["options"]["lod"], 1);
    assert_eq!(
        manifest["options"]["exclude"],
        serde_json::json!(["hardpoint_weapon_nose/hardpoint_class_2"])
    );
    let names: Vec<&str> = manifest["nodes"]
        .as_array()
        .ok_or("nodes")?
        .iter()
        .filter_map(|node| node["name"].as_str())
        .collect();
    assert!(
        !names.iter().any(|name| name.ends_with("TEST_Gun")),
        "the excluded gun is not in the scene: {names:?}"
    );
    assert!(
        names.contains(&"hardpoint_weapon_nose/TEST_Mount"),
        "{names:?}"
    );
    let ship = manifest["nodes"]
        .as_array()
        .and_then(|nodes| {
            nodes
                .iter()
                .find(|node| node["name"] == "hardpoint_weapon_nose/TEST_Mount")
        })
        .ok_or("mount")?;
    let translation: Vec<f64> = ship["translation"]
        .as_array()
        .ok_or("translation")?
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    assert!(
        translation
            .iter()
            .zip([0.0, 1.0, -4.0])
            .all(|(a, e)| (a - e).abs() < 1e-5),
        "the mount sits where entity glb puts it: {translation:?}"
    );
    Ok(())
}

#[test]
fn the_reports_count_the_models_written_at_a_lower_level_like_entity_glb() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), assembly_entries())?;

    let text = package(
        &archive,
        "TEST_Ship",
        &dir.path().join("text"),
        &["--lod", "1"],
        false,
    )?;
    let json = package(
        &archive,
        "TEST_Ship",
        &dir.path().join("json"),
        &["--lod", "1"],
        true,
    )?;

    assert_eq!(text.code, Some(0), "{}", text.stderr);
    assert!(
        text.stdout
            .contains("2 models lack LOD1 and are written at the nearest lower level"),
        "{}",
        text.stdout
    );
    let report: Value = serde_json::from_str(&json.stdout)?;
    assert_eq!(
        report["lod_fallbacks"], 2,
        "the mount and the gun have no LOD1; the relay's gun shares the gun's mesh"
    );
    Ok(())
}

#[test]
fn an_existing_package_is_replaced_and_any_other_folder_is_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), livery_entries())?;
    let out = dir.path().join("livery");
    let documents = dir.path().join("documents");
    std::fs::create_dir(&documents)?;
    std::fs::write(documents.join("thesis.txt"), b"precious")?;

    let first = package(&archive, "TEST_Livery", &out, &[], false)?;
    std::fs::write(out.join("meshes").join("m9999.glb"), b"old")?;
    let second = package(&archive, "TEST_Livery", &out, &[], false)?;
    let refused = package(&archive, "TEST_Livery", &documents, &[], false)?;

    assert_eq!(first.code, Some(0), "{}", first.stderr);
    assert_eq!(second.code, Some(0), "{}", second.stderr);
    assert!(!out.join("meshes").join("m9999.glb").exists());
    assert_eq!(refused.code, Some(2), "{}", refused.stderr);
    assert!(
        refused.stderr.contains("not a package folder"),
        "{}",
        refused.stderr
    );
    assert_eq!(std::fs::read(documents.join("thesis.txt"))?, b"precious");
    assert_eq!(
        names(dir.path())?,
        ["Data.p4k", "documents", "livery"],
        "no temporary folder remains"
    );
    Ok(())
}

#[test]
fn an_output_inside_a_protected_installation_is_invalid_input_and_writes_nothing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), livery_entries())?;
    let installation = dir.path().join("StarCitizen");
    std::fs::create_dir(&installation)?;
    let out = installation.join("LIVE").join("package");

    let output = p4kx_testkit::p4kx()?
        .env("P4KX_PROTECT_FOLDERS", &installation)
        .args(["entity", "package"])
        .arg(&archive)
        .arg("TEST_Livery")
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("never writes there"),
        "{output:?}"
    );
    assert!(names(&installation)?.is_empty(), "nothing was created");
    Ok(())
}

#[test]
fn an_output_that_is_the_archive_or_an_unknown_entity_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), livery_entries())?;
    let before = std::fs::read(&archive)?;
    let out = dir.path().join("livery");

    let same = package(&archive, "TEST_Livery", &archive, &[], false)?;
    let unknown = package(&archive, "No_Such_Ship", &out, &[], false)?;

    assert_eq!(same.code, Some(2), "{}", same.stderr);
    assert_eq!(unknown.code, Some(2), "{}", unknown.stderr);
    assert!(!out.exists(), "nothing written");
    assert_eq!(std::fs::read(&archive)?, before, "the Archive is untouched");
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
    let out = dir.path().join("ship");

    let output = p4kx_testkit::p4kx()?
        .env_remove("P4KX_ARCHIVE_KEY")
        .env(
            "P4KX_KEY_FILE",
            dir.path().join("profile").join("archive.key"),
        )
        .args(["entity", "package"])
        .arg(&archive)
        .arg("TEST_Ship")
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(7), "{output:?}");
    assert!(!out.exists(), "nothing written");
    Ok(())
}
