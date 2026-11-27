// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx interior package` end to end over the synthetic lit room of the testkit.

use std::error::Error;
use std::path::Path;

use p4kx_testkit::{
    INTERIOR_ROOT, LIT_ROOM, check_glb, interior_entries, lit_room_entries, test_key,
};
use serde_json::Value;

use crate::entity_package::{Run, manifest, write_archive};
use crate::interior_glb::assert_container_chain;

type TestResult = Result<(), Box<dyn Error>>;

fn package(
    archive: &Path,
    container: &str,
    out: &Path,
    extra: &[&str],
    json: bool,
) -> Result<Run, Box<dyn Error>> {
    package_chain(archive, &[container], out, extra, json)
}

fn package_chain(
    archive: &Path,
    containers: &[&str],
    out: &Path,
    extra: &[&str],
    json: bool,
) -> Result<Run, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    if json {
        command.arg("--json");
    }
    let output = command
        .args(["interior", "package"])
        .arg(archive)
        .args(containers)
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

fn count(manifest: &Value, kind: &str) -> Result<usize, Box<dyn Error>> {
    Ok(manifest["nodes"]
        .as_array()
        .ok_or("nodes")?
        .iter()
        .filter(|node| node["kind"] == kind)
        .count())
}

#[test]
fn the_interior_alone_is_a_package_with_containers_objects_and_lights() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), lit_room_entries())?;
    let out = dir.path().join("room");

    let run = package(&archive, LIT_ROOM, &out, &[], false)?;

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    assert!(
        run.stdout
            .contains("Interior: 2 containers, 2 objects, 5 lights"),
        "{}",
        run.stdout
    );
    let manifest = manifest(&out)?;
    let validator = jsonschema::draft202012::new(&p4kx_blender::manifest_schema())?;
    let violations: Vec<String> = validator
        .iter_errors(&manifest)
        .take(5)
        .map(|error| format!("{} at {}", error, error.instance_path()))
        .collect();
    assert!(violations.is_empty(), "{violations:?}");
    assert_eq!(count(&manifest, "entity")?, 0);
    assert_eq!(count(&manifest, "light")?, 5);
    assert_eq!(count(&manifest, "object")?, 2);
    assert!(count(&manifest, "container")? >= 1);
    let nodes = manifest["nodes"].as_array().ok_or("nodes")?;
    assert!(nodes[0]["parent"].is_null());
    assert_eq!(nodes[0]["kind"], "container");
    assert_eq!(manifest["source"]["entity"]["class_name"], "lit_room");
    assert_eq!(manifest["source"]["entity"]["guid"], "");
    for mesh in manifest["meshes"].as_array().ok_or("meshes")? {
        let file = mesh["file"].as_str().ok_or("file")?;
        check_glb(&std::fs::read(out.join(file))?)?;
    }
    Ok(())
}

#[test]
fn the_json_report_counts_what_the_package_holds() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), lit_room_entries())?;
    let out = dir.path().join("room");

    let run = package(&archive, LIT_ROOM, &out, &["--lod", "1"], true)?;

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    assert_eq!(run.stdout.lines().count(), 1, "one line of JSON");
    let report: Value = serde_json::from_str(&run.stdout)?;
    assert_eq!(report["containers"], serde_json::json!([LIT_ROOM]));
    assert_eq!(report["lod"], 1);
    assert_eq!(report["lights"], 5);
    assert_eq!(report["objects"], 2);
    assert_eq!(report["props"], 2);
    let manifest = manifest(&out)?;
    assert_eq!(
        report["nodes"].as_u64(),
        manifest["nodes"].as_array().map(|nodes| nodes.len() as u64)
    );
    assert_eq!(manifest["options"]["lod"], 1);
    Ok(())
}

#[test]
fn the_json_report_binds_selected_container_paths_to_their_directories() -> TestResult {
    for encrypted in [false, true] {
        let dir = tempfile::tempdir()?;
        let entries = interior_entries()
            .into_iter()
            .map(|entry| {
                if encrypted && entry.path() == INTERIOR_ROOT {
                    entry.encrypted()
                } else {
                    entry
                }
            })
            .collect();
        let archive = write_archive(dir.path(), entries)?;
        let out = dir.path().join("room");
        let key = test_key().to_hex();
        let chain = [INTERIOR_ROOT, "modules/cockpit.socpak"];
        let args = ["--key", key.as_str()];
        let run = package_chain(&archive, &chain, &out, &args, true)?;
        assert_eq!(run.code, Some(0), "{}", run.stderr);
        let report: Value = serde_json::from_str(&run.stdout)?;
        assert_container_chain(&report, encrypted)?;
        let uses = report
            .pointer("/provenance/uses")
            .and_then(Value::as_array)
            .ok_or("report source uses")?;
        let reported_paths = uses.iter().filter(|site| {
            site.get("scope").and_then(Value::as_str) == Some("archiveDirectoryMetadata")
                && site.get("role").and_then(Value::as_str) == Some("payload")
                && site.pointer("/location/field").and_then(Value::as_str) == Some("path")
        });
        assert_eq!(
            reported_paths.count(),
            2,
            "one directory path payload use for each opened layer"
        );

        let text_out = dir.path().join("text");
        let text = package_chain(&archive, &chain, &text_out, &args, false)?;
        assert_eq!(text.code, Some(0), "{}", text.stderr);
        assert_eq!(
            std::fs::read(out.join("scene.json"))?,
            std::fs::read(text_out.join("scene.json"))?,
            "the report does not change the emitted package manifest"
        );
    }
    Ok(())
}

#[test]
fn a_second_run_replaces_the_package_and_a_foreign_folder_is_kept() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), lit_room_entries())?;
    let out = dir.path().join("room");
    let foreign = dir.path().join("foreign");
    std::fs::create_dir(&foreign)?;
    std::fs::write(foreign.join("keep.txt"), "mine")?;

    let first = package(&archive, LIT_ROOM, &out, &[], false)?;
    let second = package(&archive, LIT_ROOM, &out, &[], false)?;
    let refused = package(&archive, LIT_ROOM, &foreign, &[], false)?;

    assert_eq!(first.code, Some(0), "{}", first.stderr);
    assert_eq!(second.code, Some(0), "{}", second.stderr);
    assert_eq!(refused.code, Some(2), "{}", refused.stderr);
    assert!(foreign.join("keep.txt").is_file());
    assert!(!foreign.join("scene.json").exists());
    Ok(())
}

#[test]
fn an_output_that_is_the_archive_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), lit_room_entries())?;

    let run = package(&archive, LIT_ROOM, &archive, &[], false)?;

    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(run.stderr.contains("is the Archive"), "{}", run.stderr);
    Ok(())
}

#[test]
fn a_container_the_archive_lacks_is_invalid_and_writes_nothing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), lit_room_entries())?;
    let out = dir.path().join("room");

    let run = package(&archive, "Data\\nothing.socpak", &out, &[], false)?;

    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(!out.exists());
    Ok(())
}

#[test]
fn an_output_inside_a_protected_installation_is_invalid_input_and_writes_nothing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), lit_room_entries())?;
    let installation = dir.path().join("StarCitizen");
    std::fs::create_dir(&installation)?;
    let out = installation.join("LIVE").join("package");

    let output = p4kx_testkit::p4kx()?
        .env("P4KX_PROTECT_FOLDERS", &installation)
        .args(["interior", "package"])
        .arg(&archive)
        .arg(LIT_ROOM)
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("never writes there"),
        "{output:?}"
    );
    assert_eq!(
        std::fs::read_dir(&installation)?.count(),
        0,
        "nothing was created"
    );
    Ok(())
}
