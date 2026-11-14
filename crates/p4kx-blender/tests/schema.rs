// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The checked-in JSON Schema of `scene.json` matches the manifest types.
//!
//! Run with `P4KX_BLESS=1` to regenerate `schemas/blender-package.schema.json`.

use std::error::Error;
use std::path::PathBuf;

type TestResult = Result<(), Box<dyn Error>>;

fn schema_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/blender-package.schema.json")
}

fn generated() -> Result<String, Box<dyn Error>> {
    let mut text = serde_json::to_string_pretty(&p4kx_blender::manifest_schema())?;
    text.push('\n');
    Ok(text)
}

#[test]
fn the_checked_in_manifest_schema_is_current() -> TestResult {
    let generated = generated()?;
    let path = schema_file();
    if std::env::var_os("P4KX_BLESS").is_some_and(|value| value == "1") {
        std::fs::write(&path, &generated)?;
    }
    let checked_in = std::fs::read_to_string(&path)?;
    assert!(
        checked_in == generated,
        "{} is stale; regenerate it with P4KX_BLESS=1 cargo test -p p4kx-blender --test schema",
        path.display()
    );
    Ok(())
}

#[test]
fn the_manifest_schema_is_draft_2020_12_and_a_valid_schema() -> TestResult {
    let schema = p4kx_blender::manifest_schema();
    assert_eq!(
        schema.get("$schema").and_then(serde_json::Value::as_str),
        Some("https://json-schema.org/draft/2020-12/schema"),
        "draft 2020-12"
    );
    jsonschema::draft202012::meta::validate(&schema).map_err(|error| error.to_string())?;
    Ok(())
}

#[test]
fn the_schema_requires_the_fields_a_reader_depends_on() {
    let schema = p4kx_blender::manifest_schema();
    let required: Vec<&str> = schema
        .get("required")
        .and_then(serde_json::Value::as_array)
        .map_or_default(|names| names.iter().filter_map(serde_json::Value::as_str).collect());
    for name in [
        "format",
        "version",
        "source",
        "coordinates",
        "nodes",
        "meshes",
        "textures",
    ] {
        assert!(required.contains(&name), "{name} is required: {required:?}");
    }
}

#[test]
fn the_collision_helpers_schema_is_current() -> TestResult {
    let schema = p4kx_blender::collision_helpers_schema();
    jsonschema::draft202012::meta::validate(&schema).map_err(|error| error.to_string())?;
    let generated = serde_json::to_string_pretty(&schema)? + "\n";
    let path = schema_file().with_file_name("blender-collision-helpers.schema.json");
    if std::env::var_os("P4KX_BLESS").is_some_and(|value| value == "1") {
        std::fs::write(&path, &generated)?;
    }
    assert_eq!(
        std::fs::read_to_string(&path)?,
        generated,
        "regenerate the collision helper schema with P4KX_BLESS=1 cargo test -p p4kx-blender --test schema",
    );
    Ok(())
}

#[test]
fn the_mesh_vertex_map_schema_is_current() -> TestResult {
    let schema = p4kx_blender::mesh_vertex_map_schema();
    jsonschema::draft202012::meta::validate(&schema).map_err(|error| error.to_string())?;
    let generated = serde_json::to_string_pretty(&schema)? + "\n";
    let path = schema_file().with_file_name("blender-mesh-vertex-map.schema.json");
    if std::env::var_os("P4KX_BLESS").is_some_and(|value| value == "1") {
        std::fs::write(&path, &generated)?;
    }
    assert_eq!(
        std::fs::read_to_string(&path)?,
        generated,
        "regenerate the mesh vertex map schema"
    );
    Ok(())
}

#[test]
fn the_mesh_vertex_map_row_schema_retains_v1_and_validates_v2_fields() -> TestResult {
    use serde_json::json;

    let validator = jsonschema::validator_for(&p4kx_blender::mesh_vertex_map_schema())?;
    for version in [1, 2] {
        let header =
            json!({"type": "header", "format": "p4kx-mesh-vertex-map", "version": version});
        assert!(validator.is_valid(&header), "version {version} header");
    }
    let mut end =
        json!({"type": "end", "meshes": "1", "vertices": "3", "sources": "1", "sourceUses": "10"});
    assert!(
        validator.is_valid(&end),
        "historical closure without a primitive count"
    );
    end["primitives"] = json!("1");
    assert!(validator.is_valid(&end), "new closure with a decimal count");
    end["primitives"] = serde_json::Value::Null;
    assert!(
        !validator.is_valid(&end),
        "only a missing or complete count"
    );
    end["primitives"] = json!(1);
    assert!(
        !validator.is_valid(&end),
        "counts remain portable decimal strings"
    );
    let mut draw = json!({
        "type": "primitive",
        "mesh": 0,
        "primitive": 0,
        "indexAccessor": null,
        "materialIndex": null,
        "materialSource": {"type": "unassigned"},
        "modelSubmesh": 0,
        "sourceSubmesh": null
    });
    assert!(validator.is_valid(&draw), "unproved optional source fields");
    for name in ["indexAccessor", "materialIndex", "sourceSubmesh"] {
        let mut missing = draw.clone();
        missing.as_object_mut().ok_or("draw object")?.remove(name);
        assert!(
            !validator.is_valid(&missing),
            "explicit nullable member {name}"
        );
    }
    draw["invented"] = json!(0);
    assert!(!validator.is_valid(&draw), "closed per-primitive row");
    Ok(())
}

#[test]
fn the_material_source_sidecar_schema_is_current() -> TestResult {
    let schema = p4kx_blender::material_source_sidecar_schema();
    jsonschema::draft202012::meta::validate(&schema).map_err(|error| error.to_string())?;
    let generated = serde_json::to_string_pretty(&schema)? + "\n";
    let path = schema_file().with_file_name("blender-material-source.schema.json");
    if std::env::var_os("P4KX_BLESS").is_some_and(|value| value == "1") {
        std::fs::write(&path, &generated)?;
    }
    assert_eq!(
        std::fs::read_to_string(&path)?,
        generated,
        "regenerate the material source schema"
    );
    Ok(())
}
