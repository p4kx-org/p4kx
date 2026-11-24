// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestCrchModel, TestEntry, TestModel, TestNode};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const MODEL: &str = "Data/Objects/Test/nodes.cgf";
const NODE_MESH_COMBO: u32 = 0x7069_7FDA;

#[test]
fn model_info_binds_ivo_node_fields_to_original_primary_bytes() -> TestResult {
    let fixture = ivo_nodes();
    let property = b"tag=branch";
    let property_start = fixture
        .windows(property.len())
        .position(|bytes| bytes == property)
        .ok_or("authored property bytes")? as u64;
    let file = TestChunkFile::ivo()
        .chunk(0x1111, 0, [0])
        .chunk(NODE_MESH_COMBO, 0, fixture);
    let start = *file.offsets().get(1).ok_or("selected node chunk")?;
    let info = archive_listing(file.bytes(), true)?;
    assert_eq!(
        info.pointer("/nodes/1/name"),
        Some(&json!("branch")),
        "selected node name"
    );
    assert_eq!(
        info.pointer("/nodes/1/parent"),
        Some(&json!(0)),
        "original parent ordinal"
    );
    assert_eq!(
        info.pointer("/nodes/1/transform/0/3"),
        Some(&json!(5.0)),
        "stored translation"
    );
    assert_eq!(
        info.pointer("/nodes/1/properties/0/value"),
        Some(&json!("branch")),
        "authored property"
    );
    assert_ivo_sites(&info, start, property_start)
}

fn ivo_nodes() -> Vec<u8> {
    TestModel::cube()
        .with_nodes(vec![
            TestNode {
                name: "root".to_owned(),
                parent: None,
                local: [[0.0; 4]; 3],
                kind: 0,
            },
            TestNode {
                name: "branch".to_owned(),
                parent: Some(0),
                kind: 2,
                local: [
                    [1.0, 0.0, 0.0, 5.0],
                    [0.0, 1.0, 0.0, 6.0],
                    [0.0, 0.0, 1.0, 7.0],
                ],
            },
        ])
        .with_node_details(1, [2.0, 3.0, 4.0], &[("tag", "branch")])
        .nmc_chunk()
}

fn assert_ivo_sites(info: &Value, start: u64, property_start: u64) -> TestResult {
    let name = Site {
        namespace: "cryEngineChunkTable",
        field: "nodeMeshTypePrefix",
        role: "selector",
        indices: &[16, 20, 16, 2],
    };
    assert_site(info, "/nodes", name, true)?;
    for (output, field, role, span) in [
        ("transform/0/3", "matrixWord", "payload", 332..336),
        ("scale/2", "scaleWord", "payload", 376..380),
        ("parent", "parent", "conversion", 388..390),
        ("helper", "geometryType", "conversion", 390..392),
        (
            "properties/0/value",
            "value",
            "conversion",
            property_start + 4..property_start + 10,
        ),
    ] {
        assert_site(
            info,
            &format!("/nodes/1/{output}"),
            Site {
                namespace: "cryEngineNodes",
                field,
                role,
                indices: &[1, 1, start + span.start, start + span.end],
            },
            true,
        )?;
    }
    let default = Site {
        namespace: "cryEngineNodes",
        field: "matrixDefault",
        role: "selector",
        indices: &[1, 0, start + 112, start + 160],
    };
    assert_site(info, "/nodes/0/transform/0/0", default, true)
}

#[test]
fn model_info_binds_crch_nodes_to_the_transposed_matrix_and_parent_id() -> TestResult {
    let file = TestCrchModel::cube().file();
    let start = *file.offsets().get(5).ok_or("child node chunk")?;
    let mut bytes = file.bytes();
    let translation = usize::try_from(start + 132)?;
    bytes
        .get_mut(translation..translation + 4)
        .ok_or("stored translation word")?
        .copy_from_slice(&9.25_f32.to_le_bytes());
    let info = archive_listing(bytes, false)?;
    assert_eq!(
        info.pointer("/nodes/1/transform/0/3"),
        Some(&json!(9.25)),
        "transposed translation"
    );
    assert_site(
        &info,
        "/nodes/1/transform/0/3",
        Site {
            namespace: "cryEngineNodes",
            field: "matrixWord",
            role: "payload",
            indices: &[5, 1, start + 132, start + 136],
        },
        false,
    )?;
    assert_eq!(
        info.pointer("/nodes/1/parent"),
        Some(&json!(0)),
        "resolved parent ordinal"
    );
    let parent_id = Site {
        namespace: "cryEngineChunkTable",
        field: "id",
        role: "selector",
        indices: &[2, 52, 56],
    };
    assert_site(&info, "/nodes/1/parent", parent_id, false)
}

#[test]
fn model_info_keeps_loose_nodes_and_their_collection_unbound() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("loose.cgf");
    fs::write(&path, TestModel::cube().single_file().cgf())?;
    let info = listing(&path, None)?;
    let unknown = json!({
        "containsEncryptedSource": null,
        "sourceSetComplete": false,
        "sourceUsesRefs": []
    });
    for path in [
        "/nodes",
        "/nodes/0/name",
        "/nodes/0/transform/0/0",
        "/nodes/0/parent",
    ] {
        assert!(info.pointer(path).is_some(), "emitted field {path}");
        assert_eq!(origin(&info, path)?, &unknown, "unbound field {path}");
    }
    assert_eq!(
        info.get("provenance"),
        Some(&json!({"sources": [], "uses": []})),
        "a loose model has no original Archive source"
    );
    Ok(())
}

fn archive_listing(bytes: Vec<u8>, encrypted: bool) -> Result<Value, Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let model = TestEntry::zstd(MODEL, bytes);
    let model = if encrypted { model.encrypted() } else { model };
    ArchiveBuilder::new().entry(model).write_to(&path)?;
    listing(&path, Some(MODEL))
}

fn listing(path: &Path, entry: Option<&str>) -> Result<Value, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command
        .args([
            "--json",
            "--key",
            &p4kx_testkit::test_key().to_hex(),
            "model",
            "info",
        ])
        .arg(path);
    if let Some(entry) = entry {
        command.arg(entry);
    }
    let output = command.output()?;
    assert!(output.status.success(), "model info: {output:?}");
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn origin<'a>(info: &'a Value, path: &str) -> Result<&'a Value, Box<dyn Error>> {
    info.get("fieldOrigins")
        .and_then(|fields| fields.get(path))
        .ok_or_else(|| format!("missing source origin at {path}").into())
}

#[derive(Clone, Copy)]
struct Site<'a> {
    namespace: &'a str,
    field: &'a str,
    role: &'a str,
    indices: &'a [u64],
}

fn assert_site(info: &Value, path: &str, site: Site<'_>, encrypted: bool) -> TestResult {
    assert!(info.pointer(path).is_some(), "emitted field {path}");
    let origin = origin(info, path)?;
    assert_eq!(
        origin
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(encrypted),
        "physical encryption for {path}"
    );
    assert_eq!(
        origin.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "complete direct node evidence for {path}"
    );
    let indices: Vec<_> = site
        .indices
        .iter()
        .map(u64::to_string)
        .map(Value::String)
        .collect();
    let refs = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("original source use references")?;
    let uses = info
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("proof inventory")?;
    let sources = info
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("physical source inventory")?;
    assert!(
        uses.iter().any(|usage| {
            usage.get("id").is_some_and(|id| refs.contains(id))
                && usage.get("role").and_then(Value::as_str) == Some(site.role)
                && usage.get("scope").and_then(Value::as_str) == Some("payload")
                && usage.pointer("/location/namespace").and_then(Value::as_str)
                    == Some(site.namespace)
                && usage.pointer("/location/field").and_then(Value::as_str) == Some(site.field)
                && usage.pointer("/location/indices").and_then(Value::as_array) == Some(&indices)
                && sources.iter().any(|owner| {
                    owner.get("id") == usage.get("source")
                        && owner
                            .pointer("/path/value")
                            .and_then(Value::as_str)
                            .is_some_and(|entry| entry.replace('\\', "/") == MODEL)
                })
        }),
        "original {} at {path} with coordinates {indices:?}",
        site.field
    );
    Ok(())
}
