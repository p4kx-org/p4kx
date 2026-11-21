// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_testkit::{
    DRESSED_HEAD_DIFFUSE, DRESSED_HEAD_MATERIAL_FILE, SplitDds, TestEntry, TestMtl,
    TestMtlMaterial, TestTextureFormat, check_glb,
};
use serde_json::Value;

use super::{TestResult, archive, dressed_chf, glb_with, textured_entries};

const HEAD_SPECULAR: &str = "Objects/Test/head_spec.tif";
const BODY_COLOUR: &str = "/character/materials/0/sub_materials/0/colors/0/rgba/0";

#[test]
fn a_chf_colour_factor_is_unknown_while_the_unmodified_png_is_clear() -> TestResult {
    let (output, material, base_image) = export(textured_entries())?;
    assert_chf_colour(&output, &material)?;
    assert_eq!(
        base_image.get("containsEncryptedSource"),
        Some(&Value::Bool(false)),
        "a plain diffuse image uses only its clear archive sources"
    );
    Ok(())
}

#[test]
fn a_specular_bake_carries_the_chf_colour_into_the_embedded_png() -> TestResult {
    let (output, material, base_image) = export(with_specular())?;
    assert_chf_colour(&output, &base_image)?;
    assert_eq!(
        material.get("containsEncryptedSource"),
        Some(&Value::Bool(false)),
        "the material's base factor is neutral when the image contains the colour"
    );
    Ok(())
}

fn with_specular() -> Vec<TestEntry> {
    let file = format!("Data\\{}", DRESSED_HEAD_MATERIAL_FILE.replace('/', "\\"));
    let mut entries = textured_entries();
    let head = TestMtlMaterial::new("shader_head")
        .shader("HumanSkin_V2")
        .diffuse([1.0; 3])
        .texture("TexSlot1", DRESSED_HEAD_DIFFUSE)
        .texture("TexSlot4", HEAD_SPECULAR)
        .public_param("SourceAverageColor", "0.5,0.25,0.125");
    entries.retain(|entry| !entry.path().eq_ignore_ascii_case(&file));
    entries.push(TestEntry::stored(
        file,
        TestMtl::multi(&[head.clone(), head.clone(), head]).cryxml(),
    ));
    entries.extend(
        SplitDds::new(TestTextureFormat::LegacyBgra8, 4, 4)
            .mips(1)
            .entries(r"Data\Objects\Test\head_spec.dds"),
    );
    entries
}

fn export(entries: Vec<TestEntry>) -> Result<(Value, Value, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path(), entries)?;
    let file = dressed_chf(dir.path())?;
    let out = dir.path().join("look.glb");
    let run = glb_with(&archive, &file, &out, true, &[])?;
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let report: Value = serde_json::from_str(&run.stdout)?;
    let written = fs::read(&out)?;
    let checked = check_glb(&written)?;
    let material = checked
        .document
        .materials()
        .find(|material| material.name() == Some("shader_head"))
        .ok_or("head material")?;
    let image = material
        .pbr_metallic_roughness()
        .base_color_texture()
        .ok_or("head base image")?
        .texture()
        .source()
        .index();
    let image = report
        .pointer(&format!("/textureImageOrigins/{image}"))
        .ok_or("image origin")?
        .clone();
    let path = format!("/materials/{}", material.index().ok_or("material index")?);
    let receipt = fs::read_to_string(out.with_file_name("look.glb.p4kx-origin.jsonl"))?;
    let material = receipt
        .lines()
        .map(serde_json::from_str::<Value>)
        .find(|row| {
            row.as_ref().is_ok_and(|row| {
                row.pointer("/address/path").and_then(Value::as_str) == Some(path.as_str())
            })
        })
        .ok_or("material field origin")??;
    Ok((report, material, image))
}

fn assert_chf_colour(report: &Value, field: &Value) -> TestResult {
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&Value::Null),
        "the actual CHF choice has unknown original lineage"
    );
    let references = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("field refs")?;
    let inputs = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("uses")?;
    assert!(
        inputs.iter().any(|source| {
            source.get("id").is_some_and(|id| references.contains(id))
                && source.pointer("/location/field").and_then(Value::as_str) == Some(BODY_COLOUR)
        }),
        "the original applied CHF RGB channel is a field contributor"
    );
    Ok(())
}
