// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, TestResult};
use crate::health_origin_assertions as proof;

const KEY: &str = "@fixture_ammo_name";
const LANGUAGES: [(Language, &str); 3] = [
    (Language::English, "Fixture rounds"),
    (Language::German, "Testmunition"),
    (Language::French, "Munitions test"),
];

#[test]
fn ammo_presentation_retains_public_name_and_original_light_transform() -> TestResult {
    let (_folder, archive) = archive()?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let row = proof::at(&catalog, "/definitions/ammoDefinitions/rows/0")?;
    check_name(&catalog, row)?;
    check_transform(row)?;
    check_light(row)?;
    proof::closure(&catalog)
}

fn check_name(catalog: &Value, row: &Value) -> TestResult {
    let name = proof::cell(row, "displayName", "present")?;
    assert_eq!(
        name.get("containsEncryptedSource"),
        Some(&json!(false)),
        "the name key reference comes from the plain DCB"
    );
    let reference = proof::at(name, "/value")?;
    let text = common::text_row(catalog, "publicNames", reference)?;
    proof::present(text, "key", &json!(KEY), Some(false))?;
    let uses = proof::uses(catalog, name)?;
    assert!(
        uses.iter().any(|used| {
            used.get("role").and_then(Value::as_str) == Some("conversion")
                && used.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeValue")
                && used.pointer("/location/field").and_then(Value::as_str) == Some("$value")
                && used.pointer("/location/indices") == Some(&json!(["0", "0"]))
        }),
        "the name reference is converted from the actual admitted Locale"
    );
    let languages = proof::at(text, "/translations/value")?;
    for (language, expected) in LANGUAGES {
        let forms = proof::at(languages, &format!("/{}", language.code()))?;
        proof::present(
            forms,
            "sourceText",
            &json!(expected),
            Some(language == Language::English),
        )?;
    }
    let protected = proof::rows(catalog, "/texts/gameTexts")?;
    assert!(
        protected
            .iter()
            .all(|text| text.pointer("/key/value") != Some(&json!(KEY))),
        "the Ammo short name has only its public domain registration"
    );
    Ok(())
}

fn check_transform(row: &Value) -> TestResult {
    let transform = proof::at(row, "/geometryTransformParams/value")?;
    proof::present(
        proof::at(transform, "/initAngles/value")?,
        "z",
        &json!({"rawBits":"40400000","finite":3.0}),
        Some(false),
    )?;
    proof::present(
        proof::at(transform, "/rotationRate/value")?,
        "x",
        &json!({"rawBits":"80000000","finite":-0.0}),
        Some(false),
    )?;
    proof::present(
        proof::at(transform, "/randomScale/value")?,
        "minimum",
        &json!({"rawBits":"00000000","finite":0.0}),
        Some(false),
    )?;
    Ok(())
}

fn check_light(row: &Value) -> TestResult {
    let light = proof::at(row, "/lightPoolParams/value")?;
    proof::present(light, "style", &json!(u8::MAX), Some(false))?;
    proof::present(light, "fake", &json!(false), Some(false))?;
    proof::present(
        light,
        "flareName",
        &json!({"type": "gameText", "sourceText": "fixture-flare"}),
        Some(false),
    )?;
    proof::present(
        proof::at(light, "/diffuseColor/value")?,
        "g",
        &json!({"rawBits":"3f000000","finite":0.5}),
        Some(false),
    )?;
    Ok(())
}

fn archive() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("fixture.p4k");
    let mut archive = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database().build()))
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ));
    for (language, value) in LANGUAGES {
        let entry = TestEntry::stored(
            language.global_ini_path(),
            format!("fixture_ammo_name={value}\n").into_bytes(),
        );
        archive = archive.entry(if language == Language::English {
            entry.encrypted()
        } else {
            entry
        });
    }
    archive.write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("AmmoParams")
                .field("displayName", TestType::Locale)
                .field(
                    "geometryTransformParams",
                    pointer("GeometryTransformParams"),
                )
                .field("lightPoolParams", pointer("PooledLightData")),
        )
        .structure(
            TestStruct::new("GeometryTransformParams")
                .field("initAngles", class("Vec3"))
                .field("rotationRate", class("Vec3"))
                .field("randomScale", class("Range")),
        )
        .structure(
            TestStruct::new("Vec3")
                .field("x", TestType::Single)
                .field("z", TestType::Single),
        )
        .structure(
            TestStruct::new("Range")
                .field("minimum", TestType::Single)
                .field("maximum", TestType::Single),
        )
        .structure(
            TestStruct::new("PooledLightData")
                .field("flareName", TestType::String)
                .field("style", TestType::UInt8)
                .field("fake", TestType::Bool)
                .field("diffuseColor", class("RGB")),
        )
        .structure(TestStruct::new("RGB").field("g", TestType::Single))
        .record(
            TestRecord::new(
                0x8140,
                "AmmoParams",
                "AmmoParams.Fixture",
                "fixture/ammo_presentation.xml",
            )
            .value("displayName", TestValue::text(KEY))
            .value("geometryTransformParams", TestValue::Struct(transform()))
            .value("lightPoolParams", TestValue::Struct(light())),
        )
}

fn transform() -> TestInstance {
    TestInstance::new("GeometryTransformParams")
        .value(
            "initAngles",
            TestValue::Struct(TestInstance::new("Vec3").value("z", TestValue::Single(3.0))),
        )
        .value(
            "rotationRate",
            TestValue::Struct(TestInstance::new("Vec3").value("x", TestValue::Single(-0.0))),
        )
        .value(
            "randomScale",
            TestValue::Struct(
                TestInstance::new("Range")
                    .value("minimum", TestValue::Single(0.0))
                    .value("maximum", TestValue::Single(1.0)),
            ),
        )
}

fn light() -> TestInstance {
    TestInstance::new("PooledLightData")
        .value("flareName", TestValue::text("fixture-flare"))
        .value("style", TestValue::UInt8(u8::MAX))
        .value("fake", TestValue::Bool(false))
        .value(
            "diffuseColor",
            TestValue::Struct(TestInstance::new("RGB").value("g", TestValue::Single(0.5))),
        )
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}
