// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::model::LodSelection;
use p4kx_extraction::{Archive, GlbOptions, ModelSource, PathIndex};
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestModel, TestMtl, TestMtlMaterial, test_key};
use serde_json::{Value, json};

use super::{BASE, MODEL, TestResult, entry_path, field_origin, field_uses, report};

const LIBRARY: &str = "Data/Objects/Test/CuBe.mtl";
const PALETTE: &str = "palette.synthetic";
const PALETTE_PATH: &str = "/library/default_palette";

#[test]
fn drawn_glb_library_proves_its_fields_with_the_original_model_selector() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("fixture.p4k");
    archive(library_bytes(), &path)?;
    let output = directory.path().join("cube.glb");
    let report = report(&path, Some(MODEL), &output)?;
    assert_loaded_values(&report);
    assert_loaded_evidence(&report)?;

    let archive = Archive::open_with_key(&path, Some(test_key()))?;
    let index = PathIndex::new(&archive);
    let unobserved = ModelSource::from_index_with(&index, MODEL, LodSelection::Only(0))?;
    let ordinary = directory.path().join("ordinary.glb");
    unobserved.write_glb_textured(&index, &GlbOptions::default(), &ordinary)?;
    assert_eq!(
        fs::read(&output)?,
        fs::read(&ordinary)?,
        "observing the drawn library preserves the written GLB"
    );
    Ok(())
}

#[test]
fn loose_glb_library_keeps_the_original_fields_explicitly_unknown() -> TestResult {
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("cube.cgf");
    let model = TestModel::cube();
    fs::write(&input, model.cgf())?;
    fs::write(directory.path().join("cube.cgfm"), model.cgfm())?;
    fs::write(directory.path().join("cube.mtl"), library_bytes())?;
    let report = report(&input, None, &directory.path().join("cube.glb"))?;
    assert_loaded_values(&report);
    for path in [
        "/model",
        "/library/path",
        "/library/target",
        "/library/single",
        PALETTE_PATH,
        "/library/authored/encoding",
    ] {
        let field = field_origin(&report, path)?;
        assert_eq!(
            field.get("containsEncryptedSource"),
            Some(&Value::Null),
            "{path} marker"
        );
        assert_eq!(
            field.get("sourceSetComplete"),
            Some(&json!(false)),
            "{path} completeness"
        );
        assert_eq!(field.get("sourceUsesRefs"), Some(&json!([])), "{path} uses");
    }
    Ok(())
}

#[test]
fn unreadable_glb_library_retains_an_attempt_without_a_payload_origin() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("unreadable.p4k");
    archive(b"invalid material bytes".to_vec(), &path)?;
    let report = report(&path, Some(MODEL), &directory.path().join("cube.glb"))?;
    assert_eq!(
        report.pointer("/library/status"),
        Some(&json!("unreadable")),
        "the existing fallback report stays unreadable"
    );
    for path in [
        "/library/status",
        "/library/problem",
        "/library/authored",
        PALETTE_PATH,
    ] {
        let field = field_origin(&report, path)?;
        assert_eq!(
            field.get("containsEncryptedSource"),
            Some(&Value::Null),
            "{path} marker"
        );
        assert_eq!(
            field.get("sourceSetComplete"),
            Some(&json!(false)),
            "{path} completeness"
        );
        for source_use in field_uses(&report, path)? {
            assert_eq!(
                entry_path(&report, source_use)?,
                MODEL,
                "only an actual model selector contributes"
            );
        }
    }
    let uses = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    let attempts = uses
        .iter()
        .filter(|source_use| source_use.get("role").and_then(Value::as_str) == Some("attempted"))
        .collect::<Vec<_>>();
    let [attempt] = attempts.as_slice() else {
        return Err("one attempted material Entry".into());
    };
    assert_eq!(
        entry_path(&report, attempt)?,
        LIBRARY,
        "the failed physical read"
    );
    assert_eq!(
        attempt.get("effectiveEncryption"),
        Some(&json!("encrypted")),
        "the attempted input keeps its own encryption declaration"
    );
    Ok(())
}

fn archive(bytes: Vec<u8>, path: &Path) -> TestResult {
    let model = TestModel::cube();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(MODEL, model.cgf()))
        .entry(TestEntry::zstd(BASE, model.cgfm()))
        .entry(TestEntry::zstd(LIBRARY, bytes).encrypted())
        .write_to(path)?;
    Ok(())
}

fn library_bytes() -> Vec<u8> {
    TestMtl::multi(&[
        TestMtlMaterial::new("unused"),
        TestMtlMaterial::new("caps").diffuse([0.8, 0.1, 0.1]),
        TestMtlMaterial::new("sides").diffuse([0.1, 0.1, 0.8]),
    ])
    .with_default_palette(PALETTE)
    .cryxml()
}

fn assert_loaded_values(report: &Value) {
    assert_eq!(
        report.pointer("/library/status"),
        Some(&json!("loaded")),
        "status"
    );
    assert_eq!(
        report.pointer("/library/path"),
        Some(&json!("objects/test/cube")),
        "authored library name"
    );
    assert_eq!(
        report.pointer("/library/single"),
        Some(&json!(false)),
        "multi-material library"
    );
    assert_eq!(
        report.pointer(PALETTE_PATH),
        Some(&json!(PALETTE)),
        "authored palette name"
    );
    assert_eq!(
        report.pointer("/library/authored/encoding"),
        Some(&json!("cry-xml")),
        "actual source encoding"
    );
    assert_eq!(
        report.get("materials"),
        Some(&json!(2)),
        "only two drawn slots"
    );
    assert!(
        report
            .get("fieldOrigins")
            .and_then(|fields| fields.get("/materials"))
            .is_none(),
        "a raw three-slot library count is not evidence for the rendered count"
    );
}

fn assert_loaded_evidence(report: &Value) -> TestResult {
    assert_eq!(
        report.get("model"),
        Some(&json!(MODEL)),
        "resolved model path"
    );
    assert_eq!(
        report.pointer("/library/target"),
        Some(&json!(LIBRARY)),
        "resolved library path"
    );
    assert_eq!(
        field_origin(report, "/model")?.get("containsEncryptedSource"),
        Some(&json!(false)),
        "the clear original directory path"
    );
    assert_loaded_path_evidence(report)?;
    for path in [
        "/library/single",
        PALETTE_PATH,
        "/library/authored/encoding",
    ] {
        let field = field_origin(report, path)?;
        assert_eq!(
            field.get("containsEncryptedSource"),
            Some(&json!(true)),
            "{path} marker"
        );
        assert_eq!(
            field.get("sourceSetComplete"),
            Some(&json!(false)),
            "{path} partial lookup"
        );
    }
    assert_palette_inputs(report)
}

fn assert_loaded_path_evidence(report: &Value) -> TestResult {
    let model_path = field_uses(report, "/model")?;
    let [model_path] = model_path.as_slice() else {
        return Err("one model path word".into());
    };
    assert_eq!(
        entry_path(report, model_path)?,
        MODEL,
        "original model path owner"
    );
    assert_eq!(
        model_path.pointer("/location/field"),
        Some(&json!("path")),
        "original model directory field"
    );
    let authored_path = field_uses(report, "/library/path")?;
    let [authored_path] = authored_path.as_slice() else {
        return Err("one original library-name field".into());
    };
    assert_eq!(
        entry_path(report, authored_path)?,
        MODEL,
        "library name comes from the model"
    );
    assert_eq!(
        authored_path.pointer("/location/field"),
        Some(&json!("materialLibrary")),
        "original library-name field"
    );
    assert_eq!(
        authored_path.get("role"),
        Some(&json!("conversion")),
        "decoded library name"
    );
    Ok(())
}

fn assert_palette_inputs(report: &Value) -> TestResult {
    let actual = field_uses(report, PALETTE_PATH)?;
    let mut selected_model = false;
    let mut authored_palette = false;
    for source_use in actual {
        match entry_path(report, source_use)? {
            MODEL => {
                assert_eq!(
                    source_use.get("role"),
                    Some(&json!("selector")),
                    "model drives the lookup"
                );
                assert_eq!(
                    source_use.pointer("/location/field"),
                    Some(&json!("materialLibrary")),
                    "original selection"
                );
                assert_eq!(
                    source_use.get("effectiveEncryption"),
                    Some(&json!("unencrypted")),
                    "the model selector is clear"
                );
                selected_model = true;
            }
            LIBRARY => {
                assert_eq!(
                    source_use.get("role"),
                    Some(&json!("payload")),
                    "selected palette supplies value"
                );
                assert_eq!(
                    source_use.pointer("/location/field"),
                    Some(&json!("DefaultPalette")),
                    "original palette attribute"
                );
                assert_eq!(
                    source_use.get("effectiveEncryption"),
                    Some(&json!("encrypted")),
                    "the selected palette's MTL is encrypted"
                );
                authored_palette = true;
            }
            other => return Err(format!("unexpected palette source {other}").into()),
        }
    }
    assert!(
        selected_model && authored_palette,
        "both original operands are bound"
    );
    Ok(())
}
