// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx interior glb` end to end over the synthetic socpak of the testkit.

use std::error::Error;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::{
    ArchiveBuilder, INTERIOR_ROOT, LIT_ROOM, TestEntry, check_glb, interior_entries,
    lit_room_entries, test_key,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

struct Run {
    stdout: String,
    stderr: String,
    code: Option<i32>,
}

fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    interior_entries()
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(path)
}

fn glb(archive: &Path, container: &str, out: &Path, json: bool) -> Result<Run, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    if json {
        command.arg("--json");
    }
    let output = command
        .args(["interior", "glb"])
        .arg(archive)
        .arg(container)
        .arg("--out")
        .arg(out)
        .output()?;
    Ok(Run {
        stdout: String::from_utf8(output.stdout)?,
        stderr: String::from_utf8(output.stderr)?,
        code: output.status.code(),
    })
}

#[test]
fn the_interior_is_written_with_its_nested_containers_and_what_is_missing_listed() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("interior.glb");

    let run = glb(&archive, INTERIOR_ROOT, &out, false)?;

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let checked = check_glb(&std::fs::read(&out)?)?;
    let cockpit = checked
        .document
        .nodes()
        .find(|node| node.name() == Some("cockpit"))
        .ok_or("no cockpit node")?;
    let (translation, _, _) = cockpit.transform().decomposed();
    assert!(
        translation
            .iter()
            .zip([0.0, 0.0, -5.0])
            .all(|(a, e)| (a - e).abs() < 1e-5),
        "{translation:?}"
    );
    assert!(run.stdout.contains("in 4 containers"), "{}", run.stdout);
    for expected in [
        "skipped ship_interior (modules/absent): the container modules/absent does not exist",
        "is nested in itself and not entered again",
        "the geometry objects/test/absent.cgf does not exist",
    ] {
        assert!(run.stdout.contains(expected), "{expected}: {}", run.stdout);
    }
    Ok(())
}

#[test]
fn the_json_report_has_a_stable_shape() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("interior.glb");

    let run = glb(&archive, INTERIOR_ROOT, &out, true)?;

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    assert_eq!(run.stdout.lines().count(), 1, "one line of JSON");
    let mut shaped: Value = serde_json::from_str(&run.stdout)?;
    for key in ["archive", "out"] {
        *shaped.get_mut(key).ok_or(key)? = Value::String(format!("<{key}>"));
    }
    let object = shaped.as_object_mut().ok_or("GLB report object")?;
    for path in ["originReceipt", "fieldOrigins", "provenance"] {
        assert!(
            object.remove(path).is_some(),
            "required report member: {path}"
        );
    }
    insta::assert_snapshot!(serde_json::to_string_pretty(&shaped)?);
    Ok(())
}

#[test]
fn fallback_for_the_nested_scene_comes_from_the_game_archive() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let fallback = TestEntry::stored(
        "Data/Materials/material_default.mtl",
        br#"<Material Name="archive-fallback" Shader="Illum" Diffuse="0.1,0.2,0.3"/>"#,
    );
    interior_entries()
        .into_iter()
        .chain([fallback])
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&archive)?;
    let out = dir.path().join("physical.glb");
    let run = glb(&archive, INTERIOR_ROOT, &out, true)?;
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let report: Value = serde_json::from_str(&run.stdout)?;
    let receipt = &report["default_material"];
    assert_eq!(receipt["state"], "resolved", "actual Archive lookup");
    assert_eq!(
        receipt["source"]["containers"],
        serde_json::json!([]),
        "root Entry ownership"
    );
    assert_eq!(
        report["default_material_ids"], 4,
        "only actual drawable fallback IDs"
    );
    let glb = check_glb(&std::fs::read(&out)?)?;
    assert!(
        glb.document
            .materials()
            .any(|material| material.name() == Some("archive-fallback")),
        "the nested scene uses the root physical material"
    );
    Ok(())
}

#[test]
fn props_and_lights_are_counted_written_and_the_unsupported_lights_listed() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    lit_room_entries()
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&archive)?;
    let out = dir.path().join("lit.glb");

    let json = glb(&archive, LIT_ROOM, &out, true)?;
    let text = glb(&archive, LIT_ROOM, &out, false)?;

    assert_eq!(json.code, Some(0), "{}", json.stderr);
    let report: Value = serde_json::from_str(&json.stdout)?;
    assert_eq!(report["props"], 2);
    assert_eq!(report["lights"], 5);
    let kinds: Vec<&str> = report["issues"]
        .as_array()
        .ok_or("no issues")?
        .iter()
        .filter_map(|issue| issue["kind"].as_str())
        .collect();
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == "light_unsupported")
            .count(),
        2,
        "Ambient and Planar: {kinds:?}"
    );
    let glb = check_glb(&std::fs::read(&out)?)?;
    assert_eq!(glb.document.lights().map_or(0, Iterator::count), 5);
    assert!(text.stdout.contains("5 lights"), "{}", text.stdout);
    assert!(
        text.stdout.contains("cannot be written as glTF lights"),
        "{}",
        text.stdout
    );
    Ok(())
}

#[test]
fn an_output_that_is_the_archive_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    let run = glb(&archive, INTERIOR_ROOT, &archive, false)?;

    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(run.stderr.contains("is the Archive"), "{}", run.stderr);
    Ok(())
}

#[test]
fn a_container_the_archive_lacks_is_invalid_and_a_non_container_is_unreadable() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("interior.glb");

    let unknown = glb(&archive, "Data\\nothing.socpak", &out, false)?;
    let not_container = glb(&archive, "Data\\objects\\test\\ship.mtl", &out, false)?;

    assert_eq!(unknown.code, Some(2), "{}", unknown.stderr);
    assert_eq!(not_container.code, Some(3), "{}", not_container.stderr);
    assert!(!out.exists());
    Ok(())
}

#[test]
fn a_selected_container_chain_binds_the_report_and_glb_to_their_actual_inputs() -> TestResult {
    let directory = tempfile::tempdir()?;
    let out = directory.path().join("interior.glb");
    let mut prior = None;
    for (label, encrypted) in [("encrypted", true), ("plain", false)] {
        let archive = directory.path().join(format!("{label}.p4k"));
        let entries = interior_entries().into_iter().map(|entry| {
            if encrypted && entry.path() == INTERIOR_ROOT {
                entry.encrypted()
            } else {
                entry
            }
        });
        entries
            .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
            .write_to(&archive)?;
        let output = p4kx_testkit::p4kx()?
            .args(["--json", "interior", "glb"])
            .arg(&archive)
            .args([INTERIOR_ROOT, "modules/cockpit.socpak", "--out"])
            .arg(&out)
            .args(["--key", &test_key().to_hex()])
            .output()?;
        assert!(
            output.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout)?;
        assert_container_chain(&report, encrypted)?;
        let bytes = std::fs::read(&out)?;
        if let Some(prior) = &prior {
            assert_eq!(&bytes, prior, "encryption differences preserve GLB bytes");
        }
        assert_receipt(&report, &out, &bytes, encrypted)?;
        prior = Some(bytes);
    }
    Ok(())
}

fn required<'a>(value: &'a Value, pointer: &str) -> Result<&'a Value, Box<dyn Error>> {
    value
        .pointer(pointer)
        .ok_or_else(|| format!("required JSON field: {pointer}").into())
}

fn assert_origin_flags(
    origin: &Value,
    encrypted: Option<bool>,
    complete: bool,
    label: &str,
) -> TestResult {
    assert_eq!(
        *required(origin, "/containsEncryptedSource")?,
        encrypted.map_or(Value::Null, Value::Bool),
        "{label} source encryption"
    );
    assert_eq!(
        *required(origin, "/sourceSetComplete")?,
        complete,
        "{label} source completeness"
    );
    Ok(())
}

fn assert_container_chain(report: &Value, encrypted: bool) -> TestResult {
    assert_eq!(
        *required(report, "/containers")?,
        serde_json::json!([INTERIOR_ROOT, r"modules\cockpit.socpak"]),
        "the CLI emits the original admitted path spelling"
    );
    let fields = required(report, "/fieldOrigins")?;
    let uses = required(report, "/provenance/uses")?
        .as_array()
        .ok_or("report original uses")?;
    let collect = fields
        .get("/containers")
        .ok_or("container collection origin")?;
    assert_origin_flags(collect, Some(encrypted), true, "/containers")?;
    for (pointer, expected) in [("/containers/0", false), ("/containers/1", encrypted)] {
        let field = fields.get(pointer).ok_or(pointer)?;
        assert_origin_flags(field, Some(expected), true, pointer)?;
        let id = required(field, "/sourceUsesRefs/0")?;
        let binding = uses
            .iter()
            .find(|row| row.get("id") == Some(id))
            .ok_or("original path use")?;
        assert_eq!(
            *required(binding, "/scope")?,
            "archiveDirectoryMetadata",
            "{pointer} directory scope"
        );
        assert_eq!(
            *required(binding, "/role")?,
            "payload",
            "{pointer} original path role"
        );
        assert_eq!(
            *required(binding, "/location/field")?,
            "path",
            "{pointer} exact path field"
        );
    }
    let triangles = fields.get("/triangles").ok_or("triangle origin")?;
    assert_origin_flags(triangles, None, false, "/triangles")
}

fn assert_receipt(report: &Value, out: &Path, bytes: &[u8], encrypted: bool) -> TestResult {
    let receipt = out.with_extension("glb.p4kx-origin.jsonl");
    let reported = required(report, "/originReceipt")?
        .as_str()
        .ok_or("reported receipt path")?;
    assert_eq!(
        Path::new(reported),
        std::fs::canonicalize(&receipt)?,
        "same published receipt"
    );
    let text = std::fs::read_to_string(receipt)?;
    let rows: Vec<Value> = text
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let mut writer = OriginDigestWriter::new(io::sink());
    writer.write_all(bytes)?;
    let digest = serde_json::to_value(writer.finish()?)?;
    let binding = rows
        .first()
        .and_then(|row| row.get("binding"))
        .ok_or("GLB digest binding")?;
    for field in ["byteLength", "sha256"] {
        assert_eq!(
            binding.get(field).ok_or(field)?,
            digest.get(field).ok_or(field)?,
            "GLB receipt {field}"
        );
    }
    let whole = rows
        .iter()
        .find(|row| row.pointer("/address/type").and_then(Value::as_str) == Some("opaquePayload"))
        .ok_or("payload origin")?;
    assert_origin_flags(whole, encrypted.then_some(true), false, "GLB payload")
}

#[test]
fn an_unowned_interior_companion_prevents_replacement() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = write_archive(directory.path())?;
    let out = directory.path().join("interior.glb");
    let receipt = out.with_extension("glb.p4kx-origin.jsonl");
    std::fs::write(&out, b"prior artifact")?;
    std::fs::write(&receipt, b"unowned companion")?;
    let result = glb(&archive, INTERIOR_ROOT, &out, true)?;
    assert_eq!(
        result.code,
        Some(1),
        "unowned companion must reject publication: {}",
        result.stderr
    );
    assert!(
        result.stdout.is_empty(),
        "refused publication cannot emit a JSON result"
    );
    assert_eq!(
        std::fs::read(&out)?,
        b"prior artifact",
        "existing output survived"
    );
    assert_eq!(
        std::fs::read(&receipt)?,
        b"unowned companion",
        "existing companion survived"
    );
    Ok(())
}
