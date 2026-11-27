// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};
use serde_json::Value;

use super::material_origin_support::{self as support, Fixture, MTL, Storage, TestResult};

const SURFACE: &str = "/data/payload/materials/0/interpreted_surface";

#[test]
fn material_plain_fields_and_literals_have_direct_complete_origins() -> TestResult {
    let fixture = Fixture::new([support::material(
        MTL,
        b"<Material Name='Hull' Diffuse='1,0,0'/>",
        Storage::Plain,
    )])?;
    for field in [
        "/data/payload/path",
        "/data/payload/single",
        "/data/payload/materials/0/name",
        "/data/payload/authored/root/name",
    ] {
        fixture.marker(MTL, field, Some(false))?;
        assert_eq!(
            fixture.field(MTL, field)?.get("sourceSetComplete"),
            Some(&Value::Bool(true)),
            "known source family has a complete immediate use set"
        );
    }
    for field in [
        "/data/record_type",
        "/basis",
        "/tinted",
        "/decal_colours",
        "/diffuse_map_is_grey",
    ] {
        let path = if field.starts_with("/data/") {
            field.to_owned()
        } else {
            format!("{SURFACE}{field}")
        };
        fixture.independent(MTL, &path)?;
    }
    Ok(())
}

#[test]
fn encrypted_material_defaults_keep_absence_and_invalid_value_selectors() -> TestResult {
    let fixture = Fixture::new([support::material(
        MTL,
        b"<Material Name='' Opacity='bad' Diffuse='1,1,1'/>",
        Storage::Encrypted,
    )])?;
    let (_, record) = fixture.record(MTL)?;
    assert_eq!(
        record.pointer(&format!("{SURFACE}/opacity/value")),
        Some(&Value::from(1.0))
    );
    for field in [
        "/name",
        "/interpreted_surface/opacity/value",
        "/interpreted_surface/shininess/raw_bits",
        "/interpreted_surface/diffuse/0/value",
    ] {
        let pointer = format!("/data/payload/materials/0{field}");
        fixture.marker(MTL, &pointer, Some(true))?;
        assert!(
            fixture.has_use(MTL, &pointer, MTL, "payload", "selector")?,
            "selected source value/default retains the actual MTL selector"
        );
    }
    fixture.independent(MTL, &format!("{SURFACE}/tinted"))
}

#[test]
fn unknown_material_slot_keeps_its_source_selector_and_original_position() -> TestResult {
    let source =
        b"<Material><SubMaterials><Unknown/><Material Name='next'/></SubMaterials></Material>";
    let fixture = Fixture::new([support::material(MTL, source, Storage::Encrypted)])?;
    let (_, record) = fixture.record(MTL)?;
    assert_eq!(
        record.pointer("/data/payload/materials/1/index"),
        Some(&Value::from(1))
    );
    assert!(
        record
            .pointer(&format!("{SURFACE}/diffuse/0/value"))
            .and_then(Value::as_f64)
            .is_some_and(|value| (value - 0.8).abs() < 0.000_001),
        "the original unknown slot uses the documented engine-default surface"
    );
    fixture.marker(MTL, &format!("{SURFACE}/diffuse/0/value"), Some(true))?;
    assert!(
        fixture.has_use(
            MTL,
            &format!("{SURFACE}/diffuse/0/value"),
            MTL,
            "payload",
            "selector"
        )?,
        "engine-default numbers depend on the source slot disposition"
    );
    fixture.independent(MTL, &format!("{SURFACE}/tinted"))
}

#[test]
fn chosen_layer_palette_does_not_claim_unused_glass_or_later_layer_inputs() -> TestResult {
    let source = b"<Material Shader='Glass' StringGenMask='%USE_TINT_PALETTE'><MatLayers><Layer PaletteTint='bad'/><Layer PaletteTint='2'/><Layer PaletteTint='3'/></MatLayers></Material>";
    let fixture = Fixture::new([support::material(MTL, source, Storage::Plain)])?;
    let pointer = format!("{SURFACE}/palette");
    assert_eq!(
        fixture
            .record(MTL)?
            .1
            .pointer(&pointer)
            .and_then(Value::as_str),
        Some("secondary")
    );
    let uses = fixture.contributions(MTL, &pointer)?;
    assert!(
        !uses.is_empty(),
        "the chosen palette retains actual source selectors"
    );
    let mut locations: Vec<_> = uses
        .iter()
        .filter(|used| {
            used.pointer("/location/field").and_then(Value::as_str) == Some("PaletteTint")
        })
        .map(|used| serde_json::to_string(&used["location"]))
        .collect::<Result<_, _>>()?;
    locations.sort();
    locations.dedup();
    assert_eq!(
        locations.len(),
        2,
        "only the invalid preceding layer and actual winning layer contribute distinct source locations"
    );
    assert!(
        !uses
            .iter()
            .filter(
                |used| used.pointer("/location/field").and_then(Value::as_str)
                    == Some("PaletteTint")
            )
            .any(|used| used
                .pointer("/location/indices")
                .and_then(Value::as_array)
                .and_then(|indices| indices.get(1))
                .and_then(Value::as_str)
                == Some("2")),
        "the later layer was never used after the winner"
    );
    assert!(
        !uses.iter().any(|used| matches!(
            used.pointer("/location/field").and_then(Value::as_str),
            Some("Shader" | "StringGenMask")
        )),
        "a winning layer short-circuits the unused glass fallback"
    );
    Ok(())
}

#[test]
fn binary_material_null_lexeme_retains_its_actual_encoding_selector() -> TestResult {
    let bytes = TestElement::new("Material")
        .attribute("Name", "binary")
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Big);
    let fixture = Fixture::new([support::material(MTL, &bytes, Storage::Encrypted)])?;
    let pointer = "/data/payload/authored/root/syntax/attributes/0/lexeme";
    assert!(
        fixture
            .record(MTL)?
            .1
            .pointer(pointer)
            .is_some_and(Value::is_null),
        "CryXML has no invented text lexeme"
    );
    fixture.marker(MTL, pointer, Some(true))?;
    assert!(
        fixture.has_use(MTL, pointer, MTL, "payload", "selector")?,
        "the generated null retains its actual source-encoding/shape selection"
    );
    assert!(
        fixture.has_use(MTL, pointer, MTL, "payload", "conversion")?,
        "the nullable syntax datum is a reader projection, not a stored null token"
    );
    assert!(
        fixture.contributions(MTL, pointer)?.iter().any(|used| used
            .get("role")
            .and_then(Value::as_str)
            == Some("selector")
            && used.pointer("/location/field").and_then(Value::as_str) == Some("encoding")),
        "the original encoding predicate is named explicitly"
    );
    assert!(
        !fixture.has_use(MTL, pointer, MTL, "payload", "payload")?,
        "binary XML has no invented stored lexeme-null payload"
    );
    assert!(
        !support::references(fixture.field(MTL, pointer)?)?.is_empty(),
        "source-selected null retains the encoding source"
    );
    Ok(())
}
