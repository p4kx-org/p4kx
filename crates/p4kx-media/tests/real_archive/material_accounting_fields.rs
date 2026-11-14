// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{Material, MaterialError, MaterialLibrary, PaletteSlot};
use p4kx_extraction::{MaterialAuthoredInfo, MaterialLibrarySourceInfo};
use p4kx_media::{Float32, MediaRecord, Status};
use serde_json::{Value, json};

use super::super::TestResult;
use super::super::shader_accounting::require;

pub(super) fn library(source: &MaterialLibrary, row: &MediaRecord) -> TestResult<bool> {
    let payload = row.data.get("payload").ok_or("actual material payload")?;
    require(
        row.subresource.namespace == "material-library",
        "material library namespace",
    )?;
    require(
        payload.get("path") == Some(&json!(source.path))
            && payload.get("single") == Some(&json!(source.single))
            && payload.get("default_palette") == Some(&json!(source.default_palette))
            && payload.get("unknown_entries") == Some(&json!(source.unknown_entries)),
        "actual root properties differ from the material reader",
    )?;
    let root = MaterialLibrarySourceInfo::of(source);
    require(
        payload.get("authored") == Some(&serde_json::to_value(&root)?),
        "actual authored root changed",
    )?;
    let materials = payload
        .get("materials")
        .and_then(Value::as_array)
        .ok_or("actual material positions")?;
    require(
        materials.len() == source.materials.len(),
        "every original slot must have a material row",
    )?;
    let mut complete = root.is_complete();
    for (index, (material, wire)) in source.materials.iter().zip(materials).enumerate() {
        complete &= authored(material, wire, index)?;
        surface(
            material,
            wire.get("interpreted_surface")
                .ok_or("actual material surface")?,
        )?;
    }
    let expected = if complete {
        Status::Complete
    } else {
        Status::Partial
    };
    require(
        row.status == expected,
        "actual material projection disposition",
    )?;
    require(
        payload.get("projection_complete") == Some(&json!(complete)),
        "source completeness flag",
    )?;
    Ok(complete)
}

fn authored(source: &Material, wire: &Value, index: usize) -> TestResult<bool> {
    let expected = MaterialAuthoredInfo::of(source);
    require(
        wire.get("index") == Some(&json!(index))
            && wire.get("name") == Some(&json!(source.name))
            && wire.get("authored") == Some(&serde_json::to_value(&expected)?),
        "actual material position, name or ordered raw fields changed",
    )?;
    Ok(expected.is_complete())
}

fn surface(source: &Material, wire: &Value) -> TestResult {
    for (field, expected) in [
        ("basis", json!("existing-reader-with-defaults")),
        ("flags", json!(source.flags.0)),
        ("shader", json!(source.shader)),
        ("surface_type", json!(source.surface_type)),
        ("string_gen_mask", json!(source.string_gen_mask)),
        ("diffuse", json!(source.diffuse.map(Float32::new))),
        ("specular", json!(source.specular.map(Float32::new))),
        ("emissive", json!(source.emissive.map(Float32::new))),
        ("shininess", json!(Float32::new(source.shininess))),
        ("opacity", json!(Float32::new(source.opacity))),
        ("alpha_test", json!(Float32::new(source.alpha_test))),
        ("glow", json!(Float32::new(source.glow))),
        (
            "palette",
            json!(source.palette.map(|slot| match slot {
                PaletteSlot::Primary => "primary",
                PaletteSlot::Secondary => "secondary",
                PaletteSlot::Tertiary => "tertiary",
                PaletteSlot::Glass => "glass",
                PaletteSlot::Decal => "decal",
                _ => "unsupported",
            })),
        ),
        ("tinted", json!(source.tinted)),
        (
            "decal_colours",
            json!(
                source
                    .decal_colours
                    .map(|colours| colours.map(|colour| colour.map(Float32::new)))
            ),
        ),
        ("diffuse_map_is_grey", json!(source.diffuse_map_is_grey)),
    ] {
        require(
            wire.get(field)
                .is_some_and(|found| equal_json(found, &expected)),
            "actual interpreted material fields changed",
        )?;
    }
    Ok(())
}

fn equal_json(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Array(actual), Value::Array(expected)) => {
            actual.len() == expected.len()
                && actual
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| equal_json(actual, expected))
        }
        (Value::Object(actual), Value::Object(expected)) => {
            actual.len() == expected.len()
                && expected.iter().all(|(key, value)| {
                    actual
                        .get(key)
                        .is_some_and(|found| equal_json(found, value))
                })
        }
        (Value::Number(actual), Value::Number(expected)) if expected.is_f64() => {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "these values were projected from exact source f32 words"
            )]
            let f32_bits =
                |number: &serde_json::Number| number.as_f64().map(|value| (value as f32).to_bits());
            f32_bits(actual) == f32_bits(expected)
        }
        _ => actual == expected,
    }
}

pub(super) fn failure(error: &MaterialError, row: &MediaRecord) -> TestResult {
    require(
        row.status == Status::Failed && row.subresource.namespace == "material-library-failure",
        "invalid original bytes require a failed material row",
    )?;
    let expected = match error {
        MaterialError::TextEncoding(_) => "text-encoding",
        MaterialError::Xml(_) => "xml",
        MaterialError::CryXml(_) => "cry-xml",
        MaterialError::TooDeep(_) => "xml-depth",
        MaterialError::NotMaterial { .. } => "not-material",
        MaterialError::SourceBytes { .. } => "source-bytes",
        MaterialError::TooMany { .. } => "typed-limit",
        MaterialError::SourceSpan => "source-span",
        MaterialError::InvalidSourceLimits => "invalid-limits",
        _ => "unsupported-reader",
    };
    require(
        row.data
            .pointer("/payload/reason/type")
            .and_then(Value::as_str)
            == Some(expected),
        "actual structured source failure reason differs",
    )
}
