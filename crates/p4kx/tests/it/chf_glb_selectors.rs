// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_testkit::gltf::mesh::Semantic;
use p4kx_testkit::{CheckedGlb, check_glb, test_key};
use serde_json::Value;

use super::{TestResult, archive, dressed_chf, glb_with, textured_entries};

#[test]
fn the_encrypted_dataforge_marks_only_values_that_use_its_selected_paths() -> TestResult {
    let dir = tempfile::tempdir()?;
    let entries = textured_entries()
        .into_iter()
        .map(|entry| {
            if entry.path().eq_ignore_ascii_case(r"Data\Game2.dcb") {
                entry.encrypted()
            } else {
                entry
            }
        })
        .collect();
    let archive = archive(dir.path(), entries)?;
    let file = dressed_chf(dir.path())?;
    let out = dir.path().join("selector.glb");
    let key = test_key().to_hex();

    let run = glb_with(&archive, &file, &out, true, &["--key", &key])?;

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let report: Value = serde_json::from_str(&run.stdout)?;
    let glb = check_glb(&fs::read(&out)?)?;
    let head_image = image_origin(&report, &glb, "shader_head")?;
    assert_selector(head_image, &report, |field| {
        field == "materialLookupTable.materials[0].filePath"
    })?;
    let hair_image = image_origin(&report, &glb, "hair_test_m")?;
    assert_selector(hair_image, &report, |field| {
        field.ends_with(".Geometry.SubGeometry[0].Geometry.Material.path")
    })?;
    let receipt = out.with_file_name("selector.glb.p4kx-origin.jsonl");
    check_unrelated_positions(&glb, &fs::read_to_string(receipt)?)?;

    let hair = report
        .get("parts")
        .and_then(Value::as_array)
        .and_then(|parts| {
            parts
                .iter()
                .position(|part| part.get("name").and_then(Value::as_str) == Some("hair_itemport"))
        })
        .ok_or("hair part")?;
    let hair_path = report_field(&report, &format!("/parts/{hair}/file"))?;
    assert_selector(hair_path, &report, |field| {
        field.ends_with(".Geometry.SubGeometry[0].Geometry.Geometry.path")
    })?;
    let body = report_field(&report, "/body")?;
    assert_eq!(
        body.get("containsEncryptedSource"),
        Some(&Value::Null),
        "body remains an independently unknown standalone CHF value"
    );
    Ok(())
}

fn image_origin<'a>(
    report: &'a Value,
    glb: &CheckedGlb,
    name: &str,
) -> Result<&'a Value, Box<dyn Error>> {
    let index = glb
        .document
        .materials()
        .find(|material| material.name() == Some(name))
        .and_then(|material| material.pbr_metallic_roughness().base_color_texture())
        .ok_or("selected base colour image")?
        .texture()
        .source()
        .index();
    report
        .pointer(&format!("/textureImageOrigins/{index}"))
        .ok_or_else(|| format!("missing rendered image origin for {name}").into())
}

fn check_unrelated_positions(glb: &CheckedGlb, receipt: &str) -> TestResult {
    let positions = glb
        .document
        .nodes()
        .find(|node| node.name() == Some("head") && node.mesh().is_some())
        .and_then(|node| node.mesh())
        .and_then(|mesh| mesh.primitives().next())
        .and_then(|primitive| primitive.get(&Semantic::Positions))
        .and_then(|positions| positions.view())
        .ok_or("head position view")?;
    let path = format!("/bufferViews/{}", positions.index());
    for row in receipt.lines().map(serde_json::from_str::<Value>) {
        let row = row?;
        if row.pointer("/address/path").and_then(Value::as_str) == Some(path.as_str()) {
            assert_eq!(
                row.get("containsEncryptedSource").and_then(Value::as_bool),
                Some(false),
                "the head's position data does not consume the encrypted material selection"
            );
            return Ok(());
        }
    }
    Err("missing addressed head geometry".into())
}

fn report_field<'a>(report: &'a Value, field: &str) -> Result<&'a Value, Box<dyn Error>> {
    report
        .get("fieldOrigins")
        .and_then(|fields| fields.get(field))
        .ok_or_else(|| format!("missing report origin for {field}").into())
}

fn assert_selector(
    origin: &Value,
    report: &Value,
    matches_field: impl Fn(&str) -> bool,
) -> TestResult {
    assert_eq!(
        origin
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "the selected source is encrypted even with clear leaf entries"
    );
    let references = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source refs")?;
    let uses = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    assert!(
        uses.iter().any(|source| {
            source.get("id").is_some_and(|id| references.contains(id))
                && source.get("role").and_then(Value::as_str) == Some("selector")
                && source.pointer("/location/type").and_then(Value::as_str) == Some("dataForge")
                && source
                    .pointer("/location/field")
                    .and_then(Value::as_str)
                    .is_some_and(&matches_field)
        }),
        "the contributing source uses name the original selected DataForge field"
    );
    Ok(())
}
