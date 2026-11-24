// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_testkit::{ArchiveBuilder, TestEntry, TestModel, p4kx, test_key};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const MODEL: &str = "Data/Objects/Test/cube.cgf";
const BASE: &str = "Data/Objects/Test/cube.cgfm";
const LOD_MODEL: &str = "Data/Objects/Test/cube_lod1.cgf";
const LOD: &str = "Data/Objects/Test/cube_lod1.cgfm";

#[test]
fn model_info_binds_original_submesh_words_to_their_actual_levels() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let model = TestModel::cube().with_lod(TestModel::cube());
    let base = model.cgfm();
    let lod = model.lod_cgfm(1).ok_or("LOD geometry")?;
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(MODEL, model.cgf()))
        .entry(TestEntry::zstd(BASE, base.clone()))
        .entry(TestEntry::zstd(
            LOD_MODEL,
            model.lod_cgf(1).ok_or("LOD model")?,
        ))
        .entry(TestEntry::zstd(LOD, lod.clone()).encrypted())
        .write_to(&path)?;
    let info = listing(&path, Some(MODEL))?;
    let levels = [(BASE, base, false), (LOD, lod, true)];
    for (level, (entry, bytes, encrypted)) in levels.iter().enumerate() {
        let submesh = format!("/lods/{level}/meshes/0/submeshes/0");
        let geometry = Geometry {
            entry,
            bytes,
            encrypted: *encrypted,
        };
        assert_word(&info, &submesh, ("material_id", "materialId", 2), &geometry)?;
        assert_word(&info, &submesh, ("index_count", "indexCount", 4), &geometry)?;
        assert_unknown(&info, &format!("{submesh}/first_index"))?;
    }
    Ok(())
}

#[test]
fn model_info_reports_unknown_origin_for_loose_submesh_fields() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("cube.cgf");
    let model = TestModel::cube();
    fs::write(&path, model.cgf())?;
    fs::write(dir.path().join("cube.cgfm"), model.cgfm())?;
    let info = listing(&path, None)?;
    let submesh = "/lods/0/meshes/0/submeshes/0";
    for name in ["material_id", "index_count", "first_index"] {
        assert_unknown(&info, &format!("{submesh}/{name}"))?;
    }
    assert_eq!(
        info.get("provenance"),
        Some(&json!({"sources": [], "uses": []}))
    );
    Ok(())
}

fn listing(path: &Path, entry: Option<&str>) -> Result<Value, Box<dyn Error>> {
    let mut command = p4kx()?;
    command
        .args(["--json", "--key", &test_key().to_hex(), "model", "info"])
        .arg(path);
    if let Some(entry) = entry {
        command.arg(entry);
    }
    let result = command.output()?;
    assert!(result.status.success(), "model info: {result:?}");
    Ok(serde_json::from_slice(&result.stdout)?)
}

fn field_origin<'a>(info: &'a Value, path: &str) -> Result<&'a Value, Box<dyn Error>> {
    assert!(info.pointer(path).is_some(), "emitted submesh field {path}");
    info.get("fieldOrigins")
        .and_then(|fields| fields.get(path))
        .ok_or_else(|| format!("missing origin at {path}").into())
}

fn assert_unknown(info: &Value, path: &str) -> TestResult {
    assert_eq!(
        field_origin(info, path)?,
        &json!({"containsEncryptedSource": null, "sourceSetComplete": false, "sourceUsesRefs": []}),
        "the evidence at {path} must stay unknown"
    );
    Ok(())
}

struct Geometry<'a> {
    entry: &'a str,
    bytes: &'a [u8],
    encrypted: bool,
}

fn assert_word(
    info: &Value,
    submesh: &str,
    field: (&str, &str, usize),
    geometry: &Geometry<'_>,
) -> TestResult {
    let (output, original, width) = field;
    let path = format!("{submesh}/{output}");
    let origin = field_origin(info, &path)?;
    assert_eq!(
        origin["containsEncryptedSource"], geometry.encrypted,
        "source at {path}"
    );
    assert_eq!(origin["sourceSetComplete"], true, "original word at {path}");
    let uses = origin["sourceUsesRefs"].as_array().ok_or("source uses")?;
    let [id] = uses.as_slice() else {
        return Err(format!("the field {path} must address one original word").into());
    };
    let use_row = origin_row(info, "/provenance/uses", id)?;
    assert_eq!(use_row["role"], "payload", "the word supplies {path}");
    let location = use_row.get("location").ok_or("original word location")?;
    assert_eq!(
        location.get("field").and_then(Value::as_str),
        Some(original),
        "the word for {path}"
    );
    assert_eq!(
        location.get("namespace").and_then(Value::as_str),
        Some("model-byte-range"),
        "the address for {path}"
    );
    let source = origin_row(info, "/provenance/sources", &use_row["source"])?;
    assert_eq!(
        source.pointer("/path/value").and_then(Value::as_str),
        Some(geometry.entry),
        "geometry for {path}"
    );
    let indices = location
        .get("indices")
        .and_then(Value::as_array)
        .ok_or("byte range")?;
    let [start, end] = indices.as_slice() else {
        return Err("expected two original byte coordinates".into());
    };
    let start: usize = start.as_str().ok_or("start decimal")?.parse()?;
    let end: usize = end.as_str().ok_or("end decimal")?.parse()?;
    assert_eq!(
        end.checked_sub(start),
        Some(width),
        "original word width at {path}"
    );
    let word = geometry.bytes.get(start..end).ok_or("geometry word")?;
    let mut widened = [0_u8; size_of::<u32>()];
    widened
        .get_mut(..width)
        .ok_or("model word width")?
        .copy_from_slice(word);
    assert_eq!(
        info.pointer(&path),
        Some(&json!(u32::from_le_bytes(widened))),
        "the original value at {path}"
    );
    Ok(())
}

fn origin_row<'a>(info: &'a Value, path: &str, id: &Value) -> Result<&'a Value, Box<dyn Error>> {
    info.pointer(path)
        .and_then(Value::as_array)
        .and_then(|rows| rows.iter().find(|row| row.get("id") == Some(id)))
        .ok_or_else(|| format!("unresolved origin id {id} in {path}").into())
}
