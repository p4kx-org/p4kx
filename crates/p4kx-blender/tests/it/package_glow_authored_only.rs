// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_blender::manifest_schema;
use p4kx_extraction::{Archive, OpenOptions};
use p4kx_testkit::test_key;
use serde_json::{Value, json};

use super::{TestResult, glow_fixture, write_glow_fixture};

#[test]
fn the_package_retains_authored_material_effects_without_recognised_glow_links() -> TestResult {
    let properties = ["SyntheticFutureGlow", "SyntheticOtherProperty"];
    let fixture = glow_fixture(&properties)?;
    let archive = Archive::open_with(fixture.path, OpenOptions::new().key(Some(test_key())))?;
    let out = tempfile::tempdir()?;
    let package = out.path().join("scene");
    write_glow_fixture(&archive, &package)?;

    let manifest: Value = serde_json::from_slice(&fs::read(package.join("scene.json"))?)?;
    jsonschema::validator_for(&manifest_schema())?
        .validate(&manifest)
        .map_err(|error| error.to_string())?;
    assert_eq!(
        manifest.pointer("/entity_glow/owners"),
        Some(&json!([{
            "node": 0,
            "declaration": 0,
            "links": [],
            "provenance": {"sources": [], "uses": []}
        }])),
        "the scene owner has no recognised geometry links or model evidence"
    );
    let declaration = manifest
        .pointer("/entity_glow/declarations/0")
        .ok_or("authored material effects")?;
    let component = declaration
        .pointer("/components/0")
        .ok_or("source component")?;
    assert_eq!(
        component["ordinal"],
        json!(1),
        "original component position"
    );
    let uses = declaration
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("original source uses")?;
    for (index, expected) in properties.into_iter().enumerate() {
        let path = format!("/tagEffects/0/propertyLinks/{index}/materialProperty");
        assert_eq!(
            component["authored"].pointer(&path).and_then(Value::as_str),
            Some(expected),
            "preserve original material-property strings in order"
        );
        let origin = component["field_origins"]
            .get(path.as_str())
            .ok_or("property origin")?;
        assert_property_origin(origin, uses, &path)?;
    }
    Ok(())
}

fn assert_property_origin(origin: &Value, uses: &[Value], path: &str) -> TestResult {
    assert_eq!(
        origin["containsEncryptedSource"],
        json!(false),
        "{path}: the property originates in the unencrypted DataForge"
    );
    assert_eq!(
        origin["sourceSetComplete"],
        json!(true),
        "{path}: the original property has complete source evidence"
    );
    let references = origin["sourceUsesRefs"]
        .as_array()
        .ok_or("source-use references")?;
    assert!(
        references.iter().any(|reference| uses.iter().any(|used| {
            used.get("id") == Some(reference)
                && used.get("role").and_then(Value::as_str) == Some("payload")
        })),
        "{path}: the retained property contributes an original payload use"
    );
    Ok(())
}
