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
