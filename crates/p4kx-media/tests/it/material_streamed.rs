// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;
use std::fs;

use p4kx_extraction::model::MaterialLibrary;
use p4kx_extraction::{MaterialAuthoredInfo, MaterialLibrarySourceInfo};
use p4kx_media::{MaterialProvider, MediaRecord, reconstruct_material_library};
use p4kx_testkit::TestEntry;
use serde::Deserialize as _;
use serde_json::{Value, json};

use super::material_support::{self as support, TestResult};

#[test]
fn multi_material_projection_above_the_default_line_budget_reconstructs_exact_values() -> TestResult
{
    let mut source = String::from("<Material xmlns:q='urn:synthetic'><SubMaterials><NoSurface/>");
    for id in 0..180 {
        append_surface(&mut source, id)?;
    }
    source.push_str("</SubMaterials></Material>");
    let options = support::options();
    let folder = tempfile::tempdir()?;
    let archive = support::archive(
        folder.path(),
        [TestEntry::stored("source.mtl", source.as_bytes())],
    )?;
    let output = folder.path().join("out");
    let registry = support::registry(MaterialProvider::default())?;
    let index = support::published(p4kx_media::export_with(
        &archive, &output, &options, &registry,
    )?)?;
    let text = fs::read_to_string(output.join(&index.artifact_base).join("material.jsonl"))?;
    let validator = jsonschema::validator_for(&p4kx_media::material_record_schema()?)?;
    let rows = read_rows(&text, &validator, options.limits.record_bytes)?;
    let recovered = reconstruct_material_library(&rows)?;
    let logical = recovered.data.get("payload").ok_or("material payload")?;
    assert!(u64::try_from(serde_json::to_vec(&recovered)?.len())? > options.limits.record_bytes);
    assert_eq!(
        logical["materials"]
            .as_array()
            .ok_or("material rows")?
            .len(),
        181
    );
    assert_eq!(
        logical["references"]
            .as_array()
            .ok_or("declarations")?
            .len(),
        360
    );
    assert_eq!(
        logical["shader_references"]
            .as_array()
            .ok_or("shader tokens")?
            .len(),
        360
    );
    compare_reader_fields(logical, source.as_bytes())?;
    assert!(validator.is_valid(&serde_json::to_value(&recovered)?));
    assert_eq!(
        reconstruct_material_library(&[recovered])?.data["record_type"],
        "library"
    );
    Ok(())
}

fn compare_reader_fields(logical: &Value, source: &[u8]) -> TestResult {
    let library = MaterialLibrary::parse("source.mtl", source)?;
    assert_eq!(
        logical.get("authored"),
        Some(&serde_json::to_value(MaterialLibrarySourceInfo::of(
            &library
        ))?),
        "the complete original syntax reconstructs from the actual packed rows"
    );
    let materials = logical
        .get("materials")
        .and_then(Value::as_array)
        .ok_or("materials")?;
    for (value, material) in materials.iter().zip(&library.materials) {
        assert_eq!(
            value.get("authored"),
            Some(&serde_json::to_value(MaterialAuthoredInfo::of(material))?),
            "all independently interpreted authored fields reconstruct"
        );
    }
    assert_eq!(
        logical.pointer("/materials/0/interpreted_surface/specular/0/value"),
        Some(&json!(0.04)),
        "the JSON value keeps the original Float32 decimal encoding"
    );
    assert_eq!(
        logical
            .pointer("/materials/0/interpreted_surface/specular/0/raw_bits")
            .and_then(Value::as_str),
        Some("3d23d70a"),
        "the same source float retains its exact IEEE-754 bits"
    );
    Ok(())
}

fn append_surface(source: &mut String, id: usize) -> Result<(), std::fmt::Error> {
    let token = "x".repeat(104);
    write!(
        source,
        "<Material Name='{id}' Shader='Synthetic' StringGenMask='%FLAG' Unknown='{token}'>        <PublicParams q:Extra='{token}' Amount='3'/><MatLayers><Layer Path='layer' Extra='{token}'/></MatLayers>        <Textures><Texture Map='Diffuse' File='tex' Extra='{token}'><TexMod Amount='1'/></Texture></Textures>        <q:Private Keep='{token}'>text<![CDATA[mixed]]><!--comment--><?check data?></q:Private></Material>"
    )
}

fn read_rows(
    text: &str,
    schema: &jsonschema::Validator,
    limit: u64,
) -> Result<Vec<MediaRecord>, Box<dyn std::error::Error>> {
    text.split_inclusive('\n')
        .map(|line| {
            assert!(
                u64::try_from(line.len())? <= limit,
                "every actual row fits the unmodified budget"
            );
            let wire: Value = serde_json::from_str(line)?;
            schema.validate(&wire).map_err(|error| error.to_string())?;
            Ok(serde_json::from_value(wire)?)
        })
        .collect()
}

#[test]
fn material_reconstruction_refuses_missing_duplicate_misordered_or_foreign_rows() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(
        folder.path(),
        [
            TestEntry::stored(
                "a.mtl",
                b"<Material Name='A'><Unknown Keep='1'/></Material>",
            ),
            TestEntry::stored(
                "b.mtl",
                b"<Material Name='B'><Unknown Keep='2'/></Material>",
            ),
        ],
    )?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &support::options())?)?;
    let wire = fs::read_to_string(output.join(index.artifact_base).join("material.jsonl"))?;
    let all: Vec<Value> = wire
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let first = all
        .iter()
        .position(|row| row["subresource"]["namespace"] == "material-library-end")
        .ok_or("first library")?
        + 1;
    let supported_first = &all[..first];
    let parse = |rows: &[Value]| -> Result<Vec<MediaRecord>, serde_json::Error> {
        rows.iter().map(MediaRecord::deserialize).collect()
    };
    let _ = reconstruct_material_library(&parse(supported_first)?)?;
    for index in 0..first {
        let mut missing = supported_first.to_vec();
        let _ = missing.remove(index);
        assert!(reconstruct_material_library(&parse(&missing)?).is_err());
    }
    for operation in [
        |rows: &mut Vec<Value>| rows.insert(2, rows[1].clone()),
        |rows: &mut Vec<Value>| rows.swap(1, 2),
        |rows: &mut Vec<Value>| rows[2]["subresource"]["indices"] = json!([99]),
    ] {
        let mut changed = supported_first.to_vec();
        operation(&mut changed);
        assert!(reconstruct_material_library(&parse(&changed)?).is_err());
    }
    let mut foreign = supported_first.to_vec();
    foreign[2].clone_from(&all[first + 2]);
    assert!(reconstruct_material_library(&parse(&foreign)?).is_err());
    Ok(())
}
