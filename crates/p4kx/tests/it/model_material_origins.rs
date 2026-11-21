// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_testkit::{ArchiveBuilder, TestEntry, TestModel, test_key};
use serde_json::{Value, json};

const MODEL: &str = "Data\\Objects\\Test\\cube.cgf";
const GEOMETRY: &str = "Data\\Objects\\Test\\cube.cgfm";
const MATERIAL: &str = "Data\\Objects\\Test\\cube.mtl";
const LAYER: &str = "Data\\Objects\\Test\\external.mtl";
const LAYER_SELECTOR: &str = "/materials/1/authored/layers/0/attributes/2/value";
const MATERIAL_XML: &str = r#"<Material xmlns:m="urn:fixture" DefaultPalette="">
    <Unknown m:Original="A&amp;B">lead<?fixture instruction?><!--sentinel--><![CDATA[tail]]><Child/></Unknown>
    <SubMaterials>
        <Empty/>
        <Material Name="caps" Diffuse="0.5,0.25,0.125">
            <Unknown/>
            <MatLayers>
                <Unknown/>
                <Layer Name="paint" Path="Objects/Test/external.mtl" Submtl="selected"/>
            </MatLayers>
        </Material>
        <Material Name="sides"/>
    </SubMaterials>
</Material>"#;

type TestResult = Result<(), Box<dyn Error>>;

fn listing(path: &Path, entry: Option<&str>) -> Result<Value, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command
        .args(["--json", "--key", &test_key().to_hex(), "model", "info"])
        .arg(path);
    if let Some(entry) = entry {
        command.arg(entry);
    }
    let output = command.output()?;
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn archive(path: &Path, encrypted_model: bool, encrypted_material: bool) -> TestResult {
    let cube = TestModel::cube();
    let model = TestEntry::zstd(MODEL, cube.cgf());
    let material = TestEntry::zstd(MATERIAL, MATERIAL_XML.as_bytes());
    ArchiveBuilder::new()
        .entry(if encrypted_model {
            model.encrypted()
        } else {
            model
        })
        .entry(TestEntry::zstd(GEOMETRY, cube.cgfm()))
        .entry(if encrypted_material {
            material.encrypted()
        } else {
            material
        })
        .entry(
            TestEntry::zstd(LAYER, b"intentionally unreadable layer".to_vec()).flagged_encrypted(),
        )
        .write_to(path)?;
    Ok(())
}

fn field_origin<'a>(info: &'a Value, pointer: &str) -> Result<&'a Value, &'static str> {
    info.get("materialFieldOrigins")
        .and_then(|origins| origins.get(pointer))
        .ok_or("material field origin")
}

fn reference_uses<'a>(info: &'a Value, field: &str) -> Result<Vec<&'a Value>, Box<dyn Error>> {
    let ids = field_origin(info, field)?
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("sourceUsesRefs")?;
    let uses = info
        .pointer("/materialProvenance/uses")
        .and_then(Value::as_array)
        .ok_or("uses")?;
    ids.iter()
        .map(|id| {
            uses.iter()
                .find(|row| row.get("id") == Some(id))
                .ok_or_else(|| "source use not found".into())
        })
        .collect()
}

fn assert_mixed_origins(
    info: &Value,
    encrypted_model: bool,
    encrypted_material: bool,
) -> TestResult {
    assert_eq!(
        info.pointer("/materials/1/diffuse"),
        Some(&json!([0.5, 0.25, 0.125])),
        "interpreted colours remain in the original output fields"
    );
    assert_eq!(
        info.pointer(LAYER_SELECTOR).and_then(Value::as_str),
        Some("selected"),
        "raw layer selector remains in the output"
    );
    let sources = info
        .pointer("/materialProvenance/sources")
        .and_then(Value::as_array)
        .ok_or("sources")?;
    assert_eq!(
        sources.len(),
        2,
        "only the original model and its selected library contributed"
    );
    for field in [
        "/materials/1/diffuse/2",
        LAYER_SELECTOR,
        "/library/default_palette",
    ] {
        let origin = field_origin(info, field)?;
        assert_eq!(
            origin
                .get("containsEncryptedSource")
                .and_then(Value::as_bool),
            Some(encrypted_model || encrypted_material),
            "combined encryption at {field}"
        );
        assert_eq!(
            origin.get("sourceSetComplete").and_then(Value::as_bool),
            Some(true),
            "all original contributors at {field}"
        );
    }
    assert_layer_uses(info, encrypted_model, encrypted_material)?;
    let palette = reference_uses(info, "/library/default_palette")?;
    assert!(
        palette.iter().any(|row| {
            row.pointer("/location/field").and_then(Value::as_str) == Some("DefaultPalette")
                && row.pointer("/location/indices") == Some(&json!(["0"]))
                && row.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("materialXmlAttribute")
        }),
        "an explicitly empty palette remains tied to its original attribute"
    );
    Ok(())
}

fn assert_layer_uses(info: &Value, encrypted_model: bool, encrypted_material: bool) -> TestResult {
    let layer = reference_uses(info, LAYER_SELECTOR)?;
    let payload = layer
        .iter()
        .find(|row| row.pointer("/location/field").and_then(Value::as_str) == Some("Submtl"))
        .ok_or("layer payload")?;
    let selector = layer
        .iter()
        .find(|row| {
            row.pointer("/location/namespace").and_then(Value::as_str) == Some("modelChunk")
        })
        .ok_or("model selector")?;
    let layer_state = if encrypted_material {
        "encrypted"
    } else {
        "unencrypted"
    };
    let model_state = if encrypted_model {
        "encrypted"
    } else {
        "unencrypted"
    };
    assert_eq!(
        payload.get("effectiveEncryption").and_then(Value::as_str),
        Some(layer_state),
        "original material Entry encryption"
    );
    assert_eq!(
        payload.get("role").and_then(Value::as_str),
        Some("payload"),
        "raw layer input"
    );
    assert_eq!(
        payload.pointer("/location/indices"),
        Some(&json!(["1", "1", "1", "1", "2"])),
        "the XML coordinate retains non-material siblings and uncompressed attribute positions"
    );
    assert_eq!(
        selector.get("effectiveEncryption").and_then(Value::as_str),
        Some(model_state),
        "original model Entry encryption"
    );
    assert_eq!(
        selector.get("role").and_then(Value::as_str),
        Some("selector"),
        "model selector"
    );
    assert_ne!(
        selector.get("source"),
        payload.get("source"),
        "model and library are distinct Entries"
    );
    Ok(())
}

#[test]
fn model_info_uses_original_model_material_and_layer_attribute_sources() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    for (model, material) in [(false, true), (true, false), (false, false)] {
        archive(&path, model, material)?;
        assert_mixed_origins(&listing(&path, Some(MODEL))?, model, material)?;
    }
    Ok(())
}

#[test]
fn model_info_disk_material_fields_have_unknown_archive_lineage() -> TestResult {
    let dir = tempfile::tempdir()?;
    let model = dir.path().join("cube.cgf");
    let cube = TestModel::cube();
    fs::write(&model, cube.cgf())?;
    fs::write(dir.path().join("cube.cgfm"), cube.cgfm())?;
    fs::write(dir.path().join("cube.mtl"), MATERIAL_XML)?;
    let info = listing(&model, None)?;
    assert_eq!(
        info.pointer("/materials/1/name").and_then(Value::as_str),
        Some("caps"),
        "disk listing remains available"
    );
    assert_eq!(
        field_origin(&info, LAYER_SELECTOR)?,
        &json!({
            "containsEncryptedSource": null,
            "sourceSetComplete": false,
            "sourceUsesRefs": []
        }),
        "disk inputs have unknown archive lineage"
    );
    assert_eq!(
        info.get("materialProvenance"),
        Some(&json!({"sources": [], "uses": []})),
        "no physical Archive source was observed"
    );
    Ok(())
}

fn assert_original_position(
    info: &Value,
    pointer: &str,
    namespace: &str,
    indices: &Value,
) -> TestResult {
    let references = reference_uses(info, pointer)?;
    assert!(
        references.iter().any(|reference| {
            reference
                .pointer("/location/namespace")
                .and_then(Value::as_str)
                == Some(namespace)
                && reference.pointer("/location/indices") == Some(indices)
        }),
        "authored XML coordinate for {pointer}"
    );
    assert_eq!(
        field_origin(info, pointer)?
            .get("sourceSetComplete")
            .and_then(Value::as_bool),
        Some(true),
        "authored XML origin completeness for {pointer}"
    );
    Ok(())
}

fn canonical_material_positions(node: &str) -> [(String, &'static str, Value); 7] {
    [
        (
            format!("{node}/syntax/attributes/0/lexeme"),
            "materialXmlDeclaration",
            json!(["0", "0"]),
        ),
        (
            format!("{node}/attributes/0/value"),
            "materialXmlAttribute",
            json!(["0", "0"]),
        ),
        (
            format!("{node}/syntax/content/1/target"),
            "materialXmlContent",
            json!(["0", "1"]),
        ),
        (
            format!("{node}/syntax/content/2/value"),
            "materialXmlContent",
            json!(["0", "2"]),
        ),
        (
            format!("{node}/children/0/name"),
            "materialXmlElement",
            json!(["0", "0"]),
        ),
        (
            "/library/authored/root/syntax/attributes/1/value".into(),
            "materialXmlDeclaration",
            json!(["1"]),
        ),
        (
            "/library/authored/entries/1/disposition".into(),
            "materialXmlElement",
            json!(["1", "1"]),
        ),
    ]
}

#[test]
fn model_info_preserves_origins_for_the_canonical_material_tree() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    archive(&path, false, true)?;
    let info = listing(&path, Some(MODEL))?;
    let node = "/library/authored/root/children/0";
    for (pointer, namespace, indices) in canonical_material_positions(node) {
        assert_original_position(&info, &pointer, namespace, &indices)?;
        assert_eq!(
            field_origin(&info, &pointer)?
                .get("containsEncryptedSource")
                .and_then(Value::as_bool),
            Some(true),
            "encrypted original material value at {pointer}"
        );
        assert!(
            info.pointer(&pointer).is_some(),
            "origin addresses an actual value at {pointer}"
        );
    }
    assert_eq!(
        info.pointer(&format!("{node}/syntax/attributes/0/lexeme"))
            .and_then(Value::as_str),
        Some("A&amp;B"),
        "authored token stays escaped in the canonical tree"
    );
    assert_eq!(
        info.pointer(&format!("{node}/attributes/0/value"))
            .and_then(Value::as_str),
        Some("A&B"),
        "decoded value stays separate from its authored token"
    );
    Ok(())
}
