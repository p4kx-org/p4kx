// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use p4kx_extraction::model::LodSelection;
use p4kx_extraction::{Archive, GlbOptions, ModelSource, OriginDigestWriter, PathIndex};
use p4kx_testkit::{
    ArchiveBuilder, TestEntry, TestModel, TestMtl, TestMtlMaterial, check_glb, test_key,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const MODEL: &str = "Data/Objects/Test/cube.cgf";
const BASE: &str = "Data/Objects/Test/cube.cgfm";
const LOD_MODEL: &str = "Data/Objects/Test/cube_lod1.cgf";
const LOD: &str = "Data/Objects/Test/cube_lod1.cgfm";

#[test]
fn standalone_receipt_binds_only_the_emitted_lod_index_counts() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive_path = dir.path().join("fixture.p4k");
    let model = TestModel::cube().with_lod(TestModel::cube());
    let base = model.cgfm();
    let lod = model.lod_cgfm(1).ok_or("LOD geometry")?;
    let material = TestMtl::multi(&[
        TestMtlMaterial::new("unused"),
        TestMtlMaterial::new("caps").diffuse([0.8, 0.1, 0.1]),
        TestMtlMaterial::new("sides").diffuse([0.1, 0.1, 0.8]),
    ]);
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(MODEL, model.cgf()))
        .entry(TestEntry::zstd(BASE, base.clone()))
        .entry(TestEntry::zstd(
            LOD_MODEL,
            model.lod_cgf(1).ok_or("LOD model")?,
        ))
        .entry(TestEntry::zstd(LOD, lod.clone()).encrypted())
        .entry(TestEntry::zstd("Data/Objects/Test/cube.mtl", material.cryxml()).encrypted())
        .write_to(&archive_path)?;
    let archive = Archive::open_with_key(&archive_path, Some(test_key()))?;
    let index = PathIndex::new(&archive);
    for (level, entry, geometry, encrypted) in [
        (0, BASE, base.as_slice(), false),
        (1, LOD, lod.as_slice(), true),
    ] {
        let output = dir.path().join(format!("level{level}.glb"));
        let ordinary = dir.path().join(format!("ordinary{level}.glb"));
        let source = ModelSource::from_index_with(&index, MODEL, LodSelection::Only(level))?;
        source.write_glb_textured(&index, &GlbOptions::default().with_lod(level), &ordinary)?;
        let command = p4kx_testkit::p4kx()?
            .args(["--json", "--key", &test_key().to_hex(), "model", "glb"])
            .arg(&archive_path)
            .arg(MODEL)
            .arg("--lod")
            .arg(level.to_string())
            .arg("--out")
            .arg(&output)
            .output()?;
        assert!(
            command.status.success(),
            "standalone GLB export: {command:?}"
        );
        let report: Value = serde_json::from_slice(&command.stdout)?;
        assert_eq!(
            report.get("originReceipt").and_then(Value::as_str),
            output
                .with_extension("glb.p4kx-origin.jsonl")
                .file_name()
                .and_then(|name| name.to_str()),
            "report addresses the canonical GLB receipt"
        );
        let bytes = fs::read(&output)?;
        assert_eq!(bytes, fs::read(ordinary)?, "standalone GLB payload bytes");
        let rows = receipt(&output, &bytes)?;
        check_counts(&bytes, &rows, entry, geometry, encrypted)?;
    }
    Ok(())
}

#[test]
fn loose_model_keeps_index_origin_unknown() -> TestResult {
    let dir = tempfile::tempdir()?;
    let model = TestModel::cube();
    let input = dir.path().join("cube.cgf");
    let out = dir.path().join("cube.glb");
    let ordinary = dir.path().join("ordinary.glb");
    fs::write(&input, model.cgf())?;
    fs::write(dir.path().join("cube.cgfm"), model.cgfm())?;
    ModelSource::from_file_with(&input, LodSelection::Only(0))?
        .write_glb(&GlbOptions::default(), &ordinary)?;
    p4kx_testkit::p4kx()?
        .args(["model", "glb"])
        .arg(&input)
        .arg("--out")
        .arg(&out)
        .assert()
        .success();
    let bytes = fs::read(&out)?;
    assert_eq!(bytes, fs::read(ordinary)?, "loose GLB payload bytes");
    let rows = receipt(&out, &bytes)?;
    let mut fields = rows
        .iter()
        .filter(|row| row.get("address").is_some())
        .peekable();
    assert!(
        fields.peek().is_some(),
        "a loose GLB still writes its origin fields"
    );
    for field in fields {
        assert_eq!(
            field.get("containsEncryptedSource"),
            Some(&Value::Null),
            "unknown loose field: {field}"
        );
        assert_eq!(
            field.get("sourceSetComplete").and_then(Value::as_bool),
            Some(false),
            "incomplete loose field: {field}"
        );
    }
    Ok(())
}

fn receipt(path: &Path, bytes: &[u8]) -> Result<Vec<Value>, Box<dyn Error>> {
    let rows = fs::read_to_string(path.with_extension("glb.p4kx-origin.jsonl"))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(bytes)?;
    let digest = serde_json::to_value(measured.finish()?)?;
    let binding = rows
        .first()
        .and_then(|row| row.get("binding"))
        .ok_or("artifact binding")?;
    assert_eq!(binding.get("sha256"), digest.get("sha256"), "GLB digest");
    assert_eq!(
        binding.get("byteLength"),
        digest.get("byteLength"),
        "GLB byte length"
    );
    Ok(rows)
}

fn check_counts(
    bytes: &[u8],
    rows: &[Value],
    entry: &str,
    geometry: &[u8],
    encrypted: bool,
) -> TestResult {
    let glb = check_glb(bytes)?;
    let accessors = glb
        .document
        .meshes()
        .flat_map(|mesh| mesh.primitives())
        .filter_map(|primitive| primitive.indices());
    let mut count = 0;
    for accessor in accessors {
        let pointer = format!("/accessors/{}/count", accessor.index());
        let field = rows
            .iter()
            .find(|row| {
                row.pointer("/address/path").and_then(Value::as_str) == Some(pointer.as_str())
            })
            .ok_or("emitted index count field")?;
        let raw_count = original_word(field, rows, entry, geometry, encrypted)?;
        assert_eq!(
            accessor.count(),
            raw_count as usize,
            "the addressed original count: {pointer}"
        );
        count += 1;
    }
    assert_eq!(count, 2, "the fixture emits both authored submeshes");
    let opaque = rows
        .iter()
        .find(|row| row.pointer("/address/type").and_then(Value::as_str) == Some("opaquePayload"))
        .ok_or("opaque GLB field")?;
    assert_eq!(
        opaque.get("sourceSetComplete").and_then(Value::as_bool),
        Some(false),
        "unproved artifact contributors stay incomplete"
    );
    assert_eq!(
        opaque.get("containsEncryptedSource"),
        Some(&if encrypted {
            Value::Bool(true)
        } else {
            Value::Null
        }),
        "only the chosen geometry can establish encrypted contribution"
    );
    Ok(())
}

fn original_word(
    field: &Value,
    rows: &[Value],
    entry: &str,
    geometry: &[u8],
    encrypted: bool,
) -> Result<u32, Box<dyn Error>> {
    assert_eq!(
        field.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "original count is bound: {field}"
    );
    assert_eq!(
        field
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(encrypted),
        "marker follows the exact selected geometry: {field}"
    );
    let uses = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("count source references")?;
    assert_eq!(uses.len(), 1, "a count uses one authored word: {field}");
    let use_row = rows
        .iter()
        .filter_map(|row| row.get("sourceUse"))
        .find(|row| row.get("id") == uses.first())
        .ok_or("count source use")?;
    assert_eq!(
        use_row.pointer("/location/field").and_then(Value::as_str),
        Some("indexCount"),
        "physical count field: {use_row}"
    );
    let source = rows
        .iter()
        .filter_map(|row| row.get("source"))
        .find(|row| row.get("id") == use_row.get("source"))
        .ok_or("count physical source")?;
    assert_eq!(
        source.pointer("/path/value").and_then(Value::as_str),
        Some(entry),
        "count belongs to the selected physical geometry Entry: {source}"
    );
    let coordinate = |index| -> Result<usize, Box<dyn Error>> {
        Ok(use_row
            .pointer(index)
            .and_then(Value::as_str)
            .ok_or("original byte coordinate")?
            .parse()?)
    };
    let start = coordinate("/location/indices/0")?;
    let end = coordinate("/location/indices/1")?;
    let bytes = geometry
        .get(start..end)
        .ok_or("count word lies in the selected Entry")?;
    Ok(u32::from_le_bytes(bytes.try_into()?))
}
