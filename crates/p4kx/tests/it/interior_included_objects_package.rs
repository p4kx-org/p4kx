// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use p4kx_extraction::OriginDigestWriter;
use serde_json::Value;

use super::{Check, read_json};

const SOC_SCHEMA: &str = include_str!("../../../../schemas/soc-scene.schema.json");

pub(super) fn verify(plain: &Path, authored: &Path) -> Check<Value> {
    let baseline = read_json(&plain.join("scene.json"))?;
    let manifest = read_json(&authored.join("scene.json"))?;
    let nodes = manifest.get("nodes").ok_or("render nodes")?;
    assert_eq!(
        baseline.get("nodes"),
        Some(nodes),
        "optional source inspection preserves the visible scene"
    );
    assert!(
        nodes.as_array().is_some_and(|nodes| nodes
            .iter()
            .any(|node| node.get("kind").and_then(Value::as_str) == Some("object"))),
        "fixture contains a real rendered object"
    );
    let relative = manifest
        .pointer("/authored_soc/file")
        .and_then(Value::as_str)
        .ok_or("companion reference")?;
    let bytes = fs::read(authored.join(relative))?;
    verify_receipt(authored, relative, &bytes)?;
    let scene: Value = serde_json::from_slice(&bytes)?;
    validate_schema(&p4kx_blender::manifest_schema(), &manifest)?;
    validate_schema(&p4kx_blender::soc_scene_schema(), &scene)?;
    validate_schema(&serde_json::from_str(SOC_SCHEMA)?, &scene)?;
    Ok(scene)
}

fn verify_receipt(package: &Path, relative: &str, bytes: &[u8]) -> Check {
    let receipt = read_json(&package.join("receipt.json"))?;
    let files = receipt
        .get("files")
        .and_then(Value::as_array)
        .ok_or("receipt files")?;
    let companion = files
        .iter()
        .find(|row| row.get("path").and_then(Value::as_str) == Some(relative))
        .ok_or("receipt for the SOC companion")?;
    let mut original = OriginDigestWriter::new(io::sink());
    original.write_all(bytes)?;
    let digest = original.finish()?;
    assert_eq!(
        companion.get("sha256").and_then(Value::as_str),
        Some(digest.sha256()),
        "receipt covers actual companion bytes"
    );
    Ok(())
}

fn validate_schema(schema: &Value, document: &Value) -> Check {
    jsonschema::validator_for(schema)?
        .validate(document)
        .map_err(|error| error.to_string())?;
    Ok(())
}
