// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use p4kx_extraction::{Archive, GlbOptions, ModelSource, OriginDigestWriter};
use p4kx_gui::{Language, ModelExport, ModelExportStatus};
use p4kx_testkit::{TestEntry, TestModel, TestMtl, TestMtlMaterial, check_glb, test_key};
use serde_json::Value;

use crate::glb_exporting::{finished, start};
use crate::headless::{opened, ready};
use crate::viewing::{Site, TestResult, opened_at, select};

const MODEL: &str = "Data/Objects/cube.cgf";
const GEOMETRY: &str = "Data/Objects/cube.cgfm";

#[test]
fn the_model_export_binds_clear_and_encrypted_geometry_to_the_unchanged_glb() -> TestResult {
    for encrypted in [false, true] {
        verify_model_export(encrypted)?;
    }
    Ok(())
}

fn verify_model_export(encrypted: bool) -> TestResult {
    let model = TestModel::cube().with_material_library("objects/cube");
    let geometry = TestEntry::zstd(GEOMETRY, model.cgfm());
    let geometry = if encrypted {
        geometry.encrypted()
    } else {
        geometry
    };
    let material = TestMtl::multi(&[
        TestMtlMaterial::new("unused"),
        TestMtlMaterial::new("caps").diffuse([0.8, 0.1, 0.1]),
        TestMtlMaterial::new("sides").diffuse([0.1, 0.1, 0.8]),
    ]);
    let entries = vec![
        TestEntry::zstd(MODEL, model.cgf()),
        geometry,
        TestEntry::zstd("Data/Objects/cube.mtl", material.cryxml()),
    ];
    let site = Site {
        entries,
        installation: false,
        key: encrypted,
    };
    let (folder, mut harness) =
        opened_at(Language::BritishEnglish, ready(), site, |builder| builder)?;
    select(&mut harness, "cube.cgf")?;
    let output = folder.path().join("cube.glb");
    assert!(start(&mut harness, &output)?, "the GLB export started");
    finished(&mut harness)?;
    let Some(ModelExportStatus::Finished(summary)) =
        harness.state().model_export().map(ModelExport::status)
    else {
        return Err("the GUI did not report a completed GLB and receipt".into());
    };
    let receipt = output.with_extension("glb.p4kx-origin.jsonl");
    assert_eq!(
        fs::canonicalize(summary.origin_receipt())?,
        fs::canonicalize(receipt)?,
        "the summary points to the sibling origin receipt"
    );
    let archive = Archive::open_with_key(folder.path().join("Data.p4k"), encrypted.then(test_key))?;
    let ordinary = folder.path().join("ordinary.glb");
    ModelSource::from_archive(&archive, MODEL)?.write_glb(&GlbOptions::default(), &ordinary)?;
    let bytes = fs::read(output)?;
    assert_eq!(
        bytes,
        fs::read(ordinary)?,
        "ordinary GLB bytes are unchanged"
    );
    check_companion(summary.origin_receipt(), &bytes, encrypted)
}

fn check_companion(receipt: &Path, bytes: &[u8], encrypted: bool) -> TestResult {
    let rows = fs::read_to_string(receipt)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    let binding = rows
        .first()
        .and_then(|row| row.get("binding"))
        .ok_or("missing artifact binding")?;
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(bytes)?;
    let digest = serde_json::to_value(measured.finish()?)?;
    assert_eq!(
        binding.get("sha256"),
        Some(digest.get("sha256").ok_or("missing measured SHA-256")?),
        "receipt binds the actual GLB bytes"
    );
    assert_eq!(
        binding.get("byteLength"),
        Some(
            digest
                .get("byteLength")
                .ok_or("missing measured byte length")?
        ),
        "receipt binds the actual GLB length"
    );

    let checked = check_glb(bytes)?;
    let mut primitives = 0;
    for mesh in checked.document.meshes() {
        for primitive in mesh.primitives() {
            let index = primitive.indices().ok_or("cube index accessor")?;
            check_field(
                &rows,
                &format!("/accessors/{}/count", index.index()),
                "indexCount",
                true,
                encrypted,
            )?;
            check_field(
                &rows,
                &format!(
                    "/meshes/{}/primitives/{}/material",
                    mesh.index(),
                    primitive.index()
                ),
                "materialId",
                false,
                encrypted,
            )?;
            primitives += 1;
        }
    }
    assert_eq!(primitives, 2, "both source submeshes were emitted");
    Ok(())
}

fn check_field(
    rows: &[Value],
    path: &str,
    field: &str,
    complete: bool,
    encrypted: bool,
) -> TestResult {
    let origin = rows
        .iter()
        .find(|row| row.pointer("/address/path").and_then(Value::as_str) == Some(path))
        .ok_or("missing original GLB field")?;
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&if complete || encrypted {
            Value::Bool(encrypted)
        } else {
            Value::Null
        }),
        "the marker reflects the observed geometry and the field's completeness: {path}"
    );
    assert_eq!(
        origin.get("sourceSetComplete").and_then(Value::as_bool),
        Some(complete),
        "source-set completeness: {path}"
    );
    let references = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("field source-use references")?;
    assert_eq!(references.len(), 1, "one selected source word: {path}");
    let use_row = rows
        .iter()
        .filter_map(|row| row.get("sourceUse"))
        .find(|source_use| source_use.get("id") == references.first())
        .ok_or("original geometry source use")?;
    assert_eq!(
        use_row.pointer("/location/field").and_then(Value::as_str),
        Some(field),
        "exact original word: {path}"
    );
    let source = rows
        .iter()
        .filter_map(|row| row.get("source"))
        .find(|source| source.get("id") == use_row.get("source"))
        .ok_or("original physical geometry input")?;
    assert_eq!(
        source.pointer("/path/value").and_then(Value::as_str),
        Some(GEOMETRY),
        "the field belongs to the selected physical geometry Entry: {path}"
    );
    Ok(())
}

#[test]
fn an_unowned_companion_leaves_the_existing_model_glb_untouched() -> TestResult {
    let (folder, mut harness) = opened(Language::BritishEnglish, ready())?;
    select(&mut harness, "cube.cgf")?;
    let output = folder.path().join("cube.glb");
    let receipt = output.with_extension("glb.p4kx-origin.jsonl");
    fs::write(&output, b"existing model")?;
    fs::write(&receipt, b"unrelated file")?;
    assert!(start(&mut harness, &output)?, "the GLB export started");
    finished(&mut harness)?;
    assert!(
        matches!(
            harness.state().model_export().map(ModelExport::status),
            Some(ModelExportStatus::Failed(_))
        ),
        "the export must report refusal when the sidecar has a different owner"
    );
    assert_eq!(
        fs::read(output)?,
        b"existing model",
        "the previous output survives"
    );
    assert_eq!(
        fs::read(receipt)?,
        b"unrelated file",
        "the unowned sibling survives"
    );
    Ok(())
}
